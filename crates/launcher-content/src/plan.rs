use std::collections::HashSet;

use launcher_net::HttpClient;
use launcher_types::{
    ContentFilter, ContentInstallPlan, ContentPlanEntry, ContentProviderId, ContentType, Instance,
    LauncherErrorPayload,
};

use crate::curseforge::{
    get_curseforge_content_type_with_base, get_curseforge_file_with_base,
    list_curseforge_versions_with_base, to_content_version as cf_to_version,
    API_BASE as CF_API_BASE,
};
use crate::modrinth::{
    get_modrinth_project_with_base, get_modrinth_version_with_base,
    list_modrinth_versions_with_base, to_content_version as mr_to_version, API_BASE as MR_API_BASE,
};

fn resolve_modrinth_version_type(project_type: ContentType, loaders: &[String]) -> ContentType {
    if project_type == ContentType::Mod
        && !loaders.is_empty()
        && loaders.iter().all(|l| l.eq_ignore_ascii_case("datapack"))
    {
        ContentType::Datapack
    } else {
        project_type
    }
}

pub async fn build_install_plan(
    http: &HttpClient,
    instance: &Instance,
    provider: ContentProviderId,
    version_id: &str,
    cf_api_key: &str,
) -> Result<ContentInstallPlan, LauncherErrorPayload> {
    build_install_plan_with_bases(
        http,
        instance,
        provider,
        version_id,
        cf_api_key,
        MR_API_BASE,
        CF_API_BASE,
    )
    .await
}

pub(crate) async fn build_install_plan_with_bases(
    http: &HttpClient,
    instance: &Instance,
    provider: ContentProviderId,
    version_id: &str,
    cf_api_key: &str,
    modrinth_base: &str,
    curseforge_base: &str,
) -> Result<ContentInstallPlan, LauncherErrorPayload> {
    let filter = ContentFilter {
        minecraft_version: Some(instance.minecraft_version.clone()),
        loader: match instance.loader.loader_type {
            launcher_types::LoaderType::Vanilla => None,
            l => Some(format!("{:?}", l).to_lowercase()),
        },
        featured: None,
    };

    let mut dependencies = Vec::new();
    let mut warnings = Vec::new();

    match provider {
        ContentProviderId::Modrinth => {
            let target_detail =
                get_modrinth_version_with_base(http, modrinth_base, version_id).await?;
            let target_project =
                get_modrinth_project_with_base(http, modrinth_base, &target_detail.project_id)
                    .await?;
            let target_type = resolve_modrinth_version_type(
                target_project.summary.content_type,
                &target_detail.loaders,
            );
            let target_version = mr_to_version(&target_detail)?;
            let downloadable = target_version.download_url.is_some();
            let mut seen_projects = HashSet::from([target_version.project_id.clone()]);

            for dep in &target_detail.dependencies {
                if dep.dependency_type == "required" {
                    if let Some(ref dep_ver_id) = dep.version_id {
                        match get_modrinth_version_with_base(http, modrinth_base, dep_ver_id).await
                        {
                            Ok(dep_detail) => match mr_to_version(&dep_detail) {
                                Ok(dep_ver) => {
                                    if !seen_projects.insert(dep_ver.project_id.clone()) {
                                        continue;
                                    }
                                    let dep_type = get_modrinth_project_with_base(
                                        http,
                                        modrinth_base,
                                        &dep_ver.project_id,
                                    )
                                    .await
                                    .map(|p| {
                                        resolve_modrinth_version_type(
                                            p.summary.content_type,
                                            &dep_detail.loaders,
                                        )
                                    })
                                    .unwrap_or(ContentType::Mod);
                                    dependencies.push(ContentPlanEntry {
                                        downloadable: dep_ver.download_url.is_some(),
                                        content_type: dep_type,
                                        version: dep_ver,
                                    });
                                }
                                Err(e) => warnings.push(format!(
                                    "Dependensi wajib '{}' gagal diproses: {}",
                                    dep_ver_id, e.message
                                )),
                            },
                            Err(e) => warnings.push(format!(
                                "Dependensi wajib '{}' gagal diambil: {}",
                                dep_ver_id, e.message
                            )),
                        }
                    } else if let Some(ref dep_proj_id) = dep.project_id {
                        if !seen_projects.insert(dep_proj_id.clone()) {
                            continue;
                        }
                        match list_modrinth_versions_with_base(
                            http,
                            modrinth_base,
                            dep_proj_id,
                            &filter,
                        )
                        .await
                        {
                            Ok(dep_versions) => {
                                if let Some(best) = dep_versions.first() {
                                    match mr_to_version(best) {
                                        Ok(dep_ver) => {
                                            let dep_type = get_modrinth_project_with_base(
                                                http,
                                                modrinth_base,
                                                dep_proj_id,
                                            )
                                            .await
                                            .map(|p| {
                                                resolve_modrinth_version_type(
                                                    p.summary.content_type,
                                                    &best.loaders,
                                                )
                                            })
                                            .unwrap_or(ContentType::Mod);
                                            dependencies.push(ContentPlanEntry {
                                                downloadable: dep_ver.download_url.is_some(),
                                                content_type: dep_type,
                                                version: dep_ver,
                                            });
                                        }
                                        Err(e) => warnings.push(format!(
                                            "Versi dependensi wajib '{}' gagal diproses: {}",
                                            dep_proj_id, e.message
                                        )),
                                    }
                                } else {
                                    warnings.push(format!(
                                        "Dependensi wajib '{}' tidak memiliki versi yang kompatibel.",
                                        dep_proj_id
                                    ));
                                }
                            }
                            Err(e) => warnings.push(format!(
                                "Dependensi wajib '{}' gagal diambil: {}",
                                dep_proj_id, e.message
                            )),
                        }
                    }
                } else if dep.dependency_type == "incompatible" {
                    warnings.push(format!(
                        "Mod ini tidak kompatibel dengan project ID: {:?}",
                        dep.project_id
                    ));
                }
            }

            Ok(ContentInstallPlan {
                target: ContentPlanEntry {
                    version: target_version,
                    content_type: target_type,
                    downloadable,
                },
                dependencies,
                warnings,
            })
        }
        ContentProviderId::Curseforge => {
            let (mod_id, file_id) = match version_id.split_once(':') {
                Some((m, f)) => (
                    m.parse::<u64>()
                        .map_err(|_| LauncherErrorPayload::invalid_input("Invalid modId"))?,
                    f.parse::<u64>()
                        .map_err(|_| LauncherErrorPayload::invalid_input("Invalid fileId"))?,
                ),
                None => {
                    return Err(LauncherErrorPayload::invalid_input(
                        "Format versionId CurseForge harus modId:fileId",
                    ))
                }
            };

            let target_file = get_curseforge_file_with_base(
                http,
                curseforge_base,
                cf_api_key,
                &mod_id.to_string(),
                file_id,
            )
            .await?;
            let target_type = get_curseforge_content_type_with_base(
                http,
                curseforge_base,
                cf_api_key,
                &mod_id.to_string(),
            )
            .await?;
            let target_version = cf_to_version(&target_file);
            let downloadable = target_version.download_url.is_some();
            let mut seen_mod_ids = HashSet::from([mod_id]);

            for dep in &target_file.dependencies {
                if dep.relation_type == 3 {
                    if !seen_mod_ids.insert(dep.mod_id) {
                        continue;
                    }
                    match list_curseforge_versions_with_base(
                        http,
                        curseforge_base,
                        cf_api_key,
                        &dep.mod_id.to_string(),
                        &filter,
                    )
                    .await
                    {
                        Ok(files) => {
                            if let Some(best) = files.first() {
                                let dep_ver = cf_to_version(best);
                                let dep_type = get_curseforge_content_type_with_base(
                                    http,
                                    curseforge_base,
                                    cf_api_key,
                                    &dep.mod_id.to_string(),
                                )
                                .await
                                .unwrap_or(ContentType::Mod);
                                dependencies.push(ContentPlanEntry {
                                    downloadable: dep_ver.download_url.is_some(),
                                    content_type: dep_type,
                                    version: dep_ver,
                                });
                            } else {
                                warnings.push(format!(
                                    "Dependensi wajib CurseForge {} tidak memiliki versi yang kompatibel.",
                                    dep.mod_id
                                ));
                            }
                        }
                        Err(e) => warnings.push(format!(
                            "Dependensi wajib CurseForge {} gagal diambil: {}",
                            dep.mod_id, e.message
                        )),
                    }
                } else if dep.relation_type == 5 {
                    warnings.push(format!(
                        "Mod ini tidak kompatibel dengan CurseForge Mod ID: {}",
                        dep.mod_id
                    ));
                }
            }

            Ok(ContentInstallPlan {
                target: ContentPlanEntry {
                    version: target_version,
                    content_type: target_type,
                    downloadable,
                },
                dependencies,
                warnings,
            })
        }
    }
}
