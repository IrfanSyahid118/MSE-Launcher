import { useState } from 'react'
import type { ReactNode } from 'react'
import {
  Boxes,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Circle,
  Compass,
  Cog,
  Feather,
  Gauge,
  ScrollText,
  ShieldCheck,
  Swords,
  Users,
  Wand2,
} from 'lucide-react'
import { useNavigate } from 'react-router'
import { useActiveAccount, useInstances } from '../../api/queries'
import styles from './RightSidebar.module.css'

export interface RightSidebarProps {
  /** Mode menentukan isi panel tengah: 'home' | 'browse' | 'project'. */
  mode?: 'home' | 'browse' | 'project'
  /** Mode browse: sembunyikan konten yang sudah terpasang. */
  hideInstalled?: boolean
  onToggleHideInstalled?: (value: boolean) => void
  /** Mode browse: kategori yang sedang dipilih. */
  selectedCategories?: string[]
  onSelectCategory?: (category: string) => void
  /** Mode project: kompatibilitas nyata dari daftar versi proyek. */
  projectCompatibility?: {
    gameVersions?: string[] | undefined
    platforms?: string[] | undefined
    environment?: string | undefined
  } | undefined
}

const CATEGORIES: { name: string; icon: ReactNode }[] = [
  { name: 'Adventure', icon: <Compass size={14} aria-hidden /> },
  { name: 'Challenging', icon: <ShieldCheck size={14} aria-hidden /> },
  { name: 'Combat', icon: <Swords size={14} aria-hidden /> },
  { name: 'Kitchen Sink', icon: <Boxes size={14} aria-hidden /> },
  { name: 'Lightweight', icon: <Feather size={14} aria-hidden /> },
  { name: 'Magic', icon: <Wand2 size={14} aria-hidden /> },
  { name: 'Multiplayer', icon: <Users size={14} aria-hidden /> },
  { name: 'Optimization', icon: <Gauge size={14} aria-hidden /> },
  { name: 'Quests', icon: <ScrollText size={14} aria-hidden /> },
  { name: 'Technology', icon: <Cog size={14} aria-hidden /> },
]

export function RightSidebar({
  mode = 'home',
  hideInstalled = false,
  onToggleHideInstalled,
  selectedCategories = [],
  onSelectCategory,
  projectCompatibility,
}: RightSidebarProps): ReactNode {
  const navigate = useNavigate()
  const { account } = useActiveAccount()
  const instancesQuery = useInstances()
  const instances = instancesQuery.data ?? []

  const [gettingStartedOpen, setGettingStartedOpen] = useState(true)
  const [categoriesOpen, setCategoriesOpen] = useState(true)

  const hasInstance = instances.length > 0
  const hasMinecraftAccount = account?.type === 'microsoft'

  const steps = [
    {
      label: 'Create first instance',
      done: hasInstance,
      action: () => navigate('/instances'),
    },
    {
      label: 'Sign in to Minecraft',
      done: hasMinecraftAccount,
      action: () => navigate('/accounts'),
    },
  ]

  return (
    <aside className={styles.sidebar} aria-label="Sidebar">
      <section className={styles.section}>
        <button
          type="button"
          className={styles.sectionHeaderBtn}
          onClick={() => setGettingStartedOpen((prev) => !prev)}
          aria-expanded={gettingStartedOpen}
        >
          <span className={styles.sectionTitle}>Getting started</span>
          {gettingStartedOpen ? <ChevronUp size={15} /> : <ChevronDown size={15} />}
        </button>

        {gettingStartedOpen ? (
          <ul className={styles.checklist}>
            {steps.map((step) => (
              <li key={step.label}>
                <button
                  type="button"
                  className={`${styles.checkItem} ${step.done ? styles.checkItemDone : ''}`}
                  onClick={step.action}
                  disabled={step.done}
                >
                  {step.done ? (
                    <CheckCircle2 size={15} className={styles.doneIcon} aria-hidden />
                  ) : (
                    <Circle size={15} className={styles.todoIcon} aria-hidden />
                  )}
                  <span>{step.label}</span>
                </button>
              </li>
            ))}
          </ul>
        ) : null}
      </section>

      {mode === 'browse' ? (
        <section className={styles.section}>
          <div className={styles.toggleRow}>
            <span className={styles.toggleLabel}>Hide already installed</span>
            <button
              type="button"
              role="switch"
              aria-checked={hideInstalled}
              aria-label="Hide already installed"
              className={`${styles.switch} ${hideInstalled ? styles.switchActive : ''}`}
              onClick={() => onToggleHideInstalled?.(!hideInstalled)}
            >
              <span className={styles.switchKnob} />
            </button>
          </div>

          <div className={styles.categoryAccordion}>
            <button
              type="button"
              className={styles.sectionHeaderBtn}
              onClick={() => setCategoriesOpen((prev) => !prev)}
              aria-expanded={categoriesOpen}
            >
              <span className={styles.sectionTitle}>Category</span>
              {categoriesOpen ? <ChevronUp size={15} /> : <ChevronDown size={15} />}
            </button>

            {categoriesOpen ? (
              <div className={styles.categoryList}>
                {CATEGORIES.map((category) => {
                  const isChecked = selectedCategories.includes(category.name)
                  return (
                    <button
                      key={category.name}
                      type="button"
                      className={`${styles.categoryItem} ${isChecked ? styles.categoryItemActive : ''}`}
                      onClick={() => onSelectCategory?.(category.name)}
                      aria-pressed={isChecked}
                    >
                      <span className={styles.categoryIcon}>{category.icon}</span>
                      <span>{category.name}</span>
                    </button>
                  )
                })}
              </div>
            ) : null}
          </div>
        </section>
      ) : null}

      {mode === 'project' ? (
        <section className={styles.section}>
          <h3 className={styles.sectionTitle}>Compatibility</h3>
          <div className={styles.compatBlock}>
            <span className={styles.compatHeading}>Minecraft: Java Edition</span>
            <div className={styles.versionChips}>
              {(projectCompatibility?.gameVersions ?? []).slice(0, 12).map((version) => (
                <span key={version} className={styles.compatChip}>
                  {version}
                </span>
              ))}
            </div>

            {projectCompatibility?.platforms && projectCompatibility.platforms.length > 0 ? (
              <>
                <span className={styles.compatHeading}>Platforms</span>
                <div className={styles.versionChips}>
                  {projectCompatibility.platforms.map((platform) => (
                    <span key={platform} className={styles.compatChip}>
                      {platform}
                    </span>
                  ))}
                </div>
              </>
            ) : null}

            {projectCompatibility?.environment ? (
              <>
                <span className={styles.compatHeading}>Supported environments</span>
                <span className={styles.compatChip}>{projectCompatibility.environment}</span>
              </>
            ) : null}
          </div>
        </section>
      ) : null}
    </aside>
  )
}
