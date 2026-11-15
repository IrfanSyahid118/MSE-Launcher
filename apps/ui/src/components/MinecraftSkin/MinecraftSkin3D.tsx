import { lazy, Suspense } from 'react'
import type { ReactNode } from 'react'
import { useI18n } from '../../i18n'
import { ErrorBoundary } from '../ErrorBoundary/ErrorBoundary'
import { SkinFallback } from './SkinFallback'

const MinecraftSkinViewer3D = lazy(async () => {
  const module = await import('./MinecraftSkinViewer3D')
  return { default: module.MinecraftSkinViewer3D }
})

export interface MinecraftSkin3DProps {
  skinUrl?: string | undefined
  capeUrl?: string | undefined
  isSlim?: boolean | undefined
  interactive?: boolean | undefined
  className?: string | undefined
  scale?: number | undefined
  autoRotateSpeed?: number | undefined
  bare?: boolean | undefined
  zoom?: number | undefined
  viewBack?: boolean | undefined
}

/**
 * Three.js 3D Minecraft Character Viewer.
 * Features 2-layer skin rendering, 360-degree mouse drag rotation,
 * and automatic idle animation.
 */
export function MinecraftSkin3D({
  skinUrl,
  capeUrl,
  isSlim,
  interactive = true,
  className,
  autoRotateSpeed,
  bare = false,
  zoom,
  viewBack,
}: MinecraftSkin3DProps): ReactNode {
  const { t } = useI18n()
  return (
    <ErrorBoundary fallback={<SkinFallback skinUrl={skinUrl} isSlim={isSlim} label={t('skin.preview2dLabel')} />}>
      <Suspense fallback={<div className={className} aria-hidden />}>
        <MinecraftSkinViewer3D
          skinUrl={skinUrl}
          capeUrl={capeUrl}
          isSlim={isSlim}
          interactive={interactive}
          className={className}
          autoRotateSpeed={autoRotateSpeed}
          bare={bare}
          zoom={zoom}
          viewBack={viewBack}
        />
      </Suspense>
    </ErrorBoundary>
  )
}
