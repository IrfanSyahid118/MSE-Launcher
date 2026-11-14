import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import styles from './SkinThumbnail.module.css'

export interface SkinThumbnailProps {
  skinUrl: string
  isSlim?: boolean
  /** Ukuran satu piksel tekstur di kanvas internal. */
  pixel?: number
  className?: string | undefined
}

interface Region {
  sx: number
  sy: number
  sw: number
  sh: number
  dx: number
  dy: number
}

/**
 * Potret depan karakter Minecraft dari tekstur 64x64. Digambar apa adanya
 * (tanpa smoothing) supaya hasilnya sama dengan kulit asli pemain.
 */
export function SkinThumbnail({
  skinUrl,
  isSlim = false,
  pixel = 4,
  className,
}: SkinThumbnailProps): ReactNode {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [broken, setBroken] = useState(false)

  useEffect(() => {
    setBroken(false)
    const canvas = canvasRef.current
    if (!canvas) return
    const context = canvas.getContext('2d')
    if (!context) return

    const image = new Image()
    image.onload = () => {
      const unit = pixel
      canvas.width = 16 * unit
      canvas.height = 32 * unit
      context.imageSmoothingEnabled = false
      context.clearRect(0, 0, canvas.width, canvas.height)

      const armWidth = isSlim ? 3 : 4
      const legY = 20
      const armY = 8

      const base: Region[] = [
        // Kaki kanan dan kiri
        { sx: 4, sy: 20, sw: 4, sh: 12, dx: 4, dy: legY },
        { sx: 20, sy: 52, sw: 4, sh: 12, dx: 8, dy: legY },
        // Lengan kanan dan kiri
        { sx: 44, sy: 20, sw: armWidth, sh: 12, dx: 0, dy: armY },
        { sx: 36, sy: 52, sw: armWidth, sh: 12, dx: 16 - armWidth, dy: armY },
        // Badan
        { sx: 20, sy: 20, sw: 8, sh: 12, dx: 4, dy: armY },
        // Kepala
        { sx: 8, sy: 8, sw: 8, sh: 8, dx: 4, dy: 0 },
      ]

      const overlay: Region[] = [
        { sx: 4, sy: 36, sw: 4, sh: 12, dx: 4, dy: legY },
        { sx: 4, sy: 52, sw: 4, sh: 12, dx: 8, dy: legY },
        { sx: 44, sy: 36, sw: armWidth, sh: 12, dx: 0, dy: armY },
        { sx: 52, sy: 52, sw: armWidth, sh: 12, dx: 16 - armWidth, dy: armY },
        { sx: 20, sy: 36, sw: 8, sh: 12, dx: 4, dy: armY },
        { sx: 40, sy: 8, sw: 8, sh: 8, dx: 4, dy: 0 },
      ]

      const drawRegions = (regions: Region[]): void => {
        for (const region of regions) {
          context.drawImage(
            image,
            region.sx,
            region.sy,
            region.sw,
            region.sh,
            region.dx * unit,
            region.dy * unit,
            region.sw * unit,
            region.sh * unit,
          )
        }
      }

      drawRegions(base)
      drawRegions(overlay)
    }
    image.onerror = () => setBroken(true)
    image.src = skinUrl
  }, [isSlim, pixel, skinUrl])

  if (broken) {
    return (
      <div className={`${styles.placeholder} ${className ?? ''}`} aria-hidden>
        <span>?</span>
      </div>
    )
  }

  return (
    <canvas
      ref={canvasRef}
      className={`${styles.canvas} ${className ?? ''}`}
      aria-hidden
      data-testid="skin-thumbnail"
    />
  )
}
