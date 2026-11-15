import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import type { Account } from '@mc/shared'
import { getAlexSkinDataUrl, getSteveSkinDataUrl } from './skin-textures'
import { useActiveSkin } from './useActiveSkin'
import styles from './PlayerHeadAvatar.module.css'

export interface PlayerHeadAvatarProps {
  account: Account
  /** Opsional untuk kompatibilitas pemanggil lama; resolusi skin kini otomatis per akun. */
  isActiveAccount?: boolean
  size?: number
  className?: string | undefined
}

const ISO_COS30 = 0.8660254
const FACE_UPSCALE = 64

function renderShadedFace(
  image: HTMLImageElement,
  sx: number,
  sy: number,
  sw: number,
  shadeAlpha: number,
  scratch: HTMLCanvasElement,
): HTMLCanvasElement | null {
  if (sx + sw > image.width || sy + sw > image.height) return null
  scratch.width = FACE_UPSCALE
  scratch.height = FACE_UPSCALE
  const fctx = scratch.getContext('2d')
  if (!fctx) return null
  fctx.globalCompositeOperation = 'source-over'
  fctx.clearRect(0, 0, FACE_UPSCALE, FACE_UPSCALE)
  fctx.imageSmoothingEnabled = false
  fctx.drawImage(image, sx, sy, sw, sw, 0, 0, FACE_UPSCALE, FACE_UPSCALE)
  if (shadeAlpha > 0) {
    fctx.globalCompositeOperation = 'source-atop'
    fctx.fillStyle = `rgba(0, 0, 0, ${shadeAlpha})`
    fctx.fillRect(0, 0, FACE_UPSCALE, FACE_UPSCALE)
    fctx.globalCompositeOperation = 'source-over'
  }
  return scratch
}

function drawIsoCubeLayer(
  ctx: CanvasRenderingContext2D,
  image: HTMLImageElement,
  unit: number,
  cx: number,
  cy: number,
  radius: number,
  isOverlay: boolean,
  scratch: HTMLCanvasElement,
): void {
  const dx = radius * ISO_COS30
  const dy = radius * 0.5
  const h = radius
  const faceSize = 8 * unit
  const bx = isOverlay ? 32 * unit : 0
  const span = isOverlay ? 1.005 : 1.015

  const topFace = renderShadedFace(image, bx + 8 * unit, 0, faceSize, 0, scratch)
  if (topFace) {
    ctx.setTransform(dx, dy, -dx, dy, cx, cy - 2 * dy)
    ctx.drawImage(topFace, 0, 0, span, span)
  }

  const frontFace = renderShadedFace(image, bx + 8 * unit, 8 * unit, faceSize, 0.06, scratch)
  if (frontFace) {
    ctx.setTransform(dx, dy, 0, h, cx - dx, cy - dy)
    ctx.drawImage(frontFace, 0, 0, span, span)
  }

  const sideFace = renderShadedFace(image, bx + 16 * unit, 8 * unit, faceSize, 0.22, scratch)
  if (sideFace) {
    ctx.setTransform(dx, -dy, 0, h, cx, cy)
    ctx.drawImage(sideFace, 0, 0, span, span)
  }
}

function drawIsometricHead(
  canvas: HTMLCanvasElement,
  context: CanvasRenderingContext2D,
  image: HTMLImageElement,
  targetSize: number,
): void {
  const drawSize = Math.max(128, Math.round(targetSize * 4))
  canvas.width = drawSize
  canvas.height = drawSize

  const unit = Math.max(1, Math.floor(image.width / 64))
  const cx = drawSize / 2
  const cy = drawSize / 2
  const baseRadius = drawSize * 0.39
  const overlayRadius = drawSize * 0.44
  const scratch = document.createElement('canvas')

  context.setTransform(1, 0, 0, 1, 0, 0)
  context.clearRect(0, 0, drawSize, drawSize)
  context.imageSmoothingEnabled = false

  drawIsoCubeLayer(context, image, unit, cx, cy, baseRadius, false, scratch)
  drawIsoCubeLayer(context, image, unit, cx, cy, overlayRadius, true, scratch)
  context.setTransform(1, 0, 0, 1, 0, 0)
}

/**
 * Avatar kepala 3D isometrik karakter Minecraft untuk profil akun.
 * - Akun Microsoft merender kepala 3D dari skin resmi Mojang yang tersimpan pada akun (atau mc-heads jika belum tercache).
 * - Akun offline memakai kepala default Steve/Alex.
 * - Jika pemain mengunggah skin custom, kepala mengikuti skin tersebut.
 */
export function PlayerHeadAvatar({
  account,
  size = 24,
  className,
}: PlayerHeadAvatarProps): ReactNode {
  const activeSkin = useActiveSkin(account)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [canvasFallbackUsed, setCanvasFallbackUsed] = useState<string | null>(null)

  const fallbackSkinUrl = activeSkin.isSlim ? getAlexSkinDataUrl() : getSteveSkinDataUrl()
  const canvasSkinUrl = activeSkin.skinUrl
  const effectiveCanvasSrc =
    canvasFallbackUsed === canvasSkinUrl ? fallbackSkinUrl : canvasSkinUrl

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const context = canvas.getContext('2d')
    if (!context) return

    let cancelled = false
    const image = new Image()
    if (
      effectiveCanvasSrc.startsWith('http://') ||
      effectiveCanvasSrc.startsWith('https://')
    ) {
      image.crossOrigin = 'anonymous'
    }

    image.onload = () => {
      if (cancelled) return
      try {
        drawIsometricHead(canvas, context, image, size)
      } catch {
        if (!cancelled && effectiveCanvasSrc !== fallbackSkinUrl) {
          setCanvasFallbackUsed(canvasSkinUrl)
        }
      }
    }
    image.onerror = () => {
      if (cancelled) return
      if (effectiveCanvasSrc !== fallbackSkinUrl) {
        setCanvasFallbackUsed(canvasSkinUrl)
      }
    }
    image.src = effectiveCanvasSrc

    return () => {
      cancelled = true
    }
  }, [canvasSkinUrl, effectiveCanvasSrc, fallbackSkinUrl, size])

  const dimensionStyle = { width: size, height: size }

  return (
    <span
      className={[styles.root, className ?? ''].filter(Boolean).join(' ')}
      style={dimensionStyle}
      aria-hidden
    >
      <canvas ref={canvasRef} className={styles.canvas} />
    </span>
  )
}
