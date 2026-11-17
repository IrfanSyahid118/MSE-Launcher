/**
 * Definisi tekstur skin Minecraft 64x64 standar untuk rendering 3D.
 * Dilengkapi dengan tekstur asli Steve & Alex resmi Mojang (Base64 PNG 64x64)
 * dan ekstraksi 2-layer (Base Body + Outer Overlay 3D) untuk topi, rambut luar,
 * jaket, lengan baju, dan celana luar.
 */

export interface SkinFaces {
  headFront: string
  headBack: string
  headLeft: string
  headRight: string
  headTop: string
  headBottom: string

  torsoFront: string
  torsoBack: string
  torsoLeft: string
  torsoRight: string
  torsoTop: string
  torsoBottom: string

  armRightFront: string
  armRightBack: string
  armRightLeft: string
  armRightRight: string
  armRightTop: string
  armRightBottom: string

  armLeftFront: string
  armLeftBack: string
  armLeftLeft: string
  armLeftRight: string
  armLeftTop: string
  armLeftBottom: string

  legRightFront: string
  legRightBack: string
  legRightLeft: string
  legRightRight: string
  legRightTop: string
  legRightBottom: string

  legLeftFront: string
  legLeftBack: string
  legLeftLeft: string
  legLeftRight: string
  legLeftTop: string
  legLeftBottom: string
}

export interface OverlayVisibility {
  head: boolean
  torso: boolean
  armRight: boolean
  armLeft: boolean
  legRight: boolean
  legLeft: boolean
}

export interface ExtractedSkinResult {
  faces: SkinFaces
  baseFaces: SkinFaces
  overlayFaces: SkinFaces
  hasOverlay: OverlayVisibility
  isSlim: boolean
}

/** Tekstur resmi 64x64 Steve Minecraft Java Edition (Base64 PNG). */
export const OFFICIAL_STEVE_SKIN =
  'data:image/png;base64,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'

/** Tekstur resmi 64x64 Alex Minecraft Java Edition (Base64 PNG). */
export const OFFICIAL_ALEX_SKIN =
  'data:image/png;base64,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'

export function getSteveSkinDataUrl(): string {
  return OFFICIAL_STEVE_SKIN
}

export function getAlexSkinDataUrl(): string {
  return OFFICIAL_ALEX_SKIN
}

const MAX_FACES_CACHE = 8
const facesCache = new Map<string, ExtractedSkinResult>()

function setFacesCache(key: string, value: ExtractedSkinResult): void {
  if (facesCache.size >= MAX_FACES_CACHE) {
    const oldest = facesCache.keys().next().value
    if (oldest) facesCache.delete(oldest)
  }
  facesCache.set(key, value)
}

interface FaceSlice {
  url: string
  hasPixels: boolean
}

/**
 * Mengiris satu permukaan wajah dari kanvas sumber.
 * Memeriksa apakah ada piksel terlihat (alpha > 0) untuk layer overlay.
 */
function sliceFace(
  sourceCanvas: HTMLCanvasElement,
  rect: { x: number; y: number; w: number; h: number },
  isOverlay = false,
): FaceSlice {
  const c = document.createElement('canvas')
  c.width = rect.w
  c.height = rect.h
  const ctx = c.getContext('2d')
  if (!ctx) return { url: '', hasPixels: false }

  ctx.imageSmoothingEnabled = false
  if (rect.y + rect.h <= sourceCanvas.height && rect.x + rect.w <= sourceCanvas.width) {
    ctx.drawImage(sourceCanvas, rect.x, rect.y, rect.w, rect.h, 0, 0, rect.w, rect.h)
  }

  let hasPixels = false
  try {
    const data = ctx.getImageData(0, 0, rect.w, rect.h).data
    for (let i = 3; i < data.length; i += 4) {
      if ((data[i] ?? 0) > 0) {
        hasPixels = true
        break
      }
    }
  } catch {
    hasPixels = true
  }

  if (isOverlay && !hasPixels) {
    return { url: '', hasPixels: false }
  }

  return { url: c.toDataURL('image/png'), hasPixels }
}

/**
 * Deteksi model ramping (Alex / Slim 3px) berdasarkan transparansi pada area lengan kanan.
 */
function checkSlimModel(canvas: HTMLCanvasElement): boolean {
  if (canvas.height < 64) return false
  const ctx = canvas.getContext('2d')
  if (!ctx) return false
  try {
    const data = ctx.getImageData(55, 20, 1, 1).data
    const data2 = ctx.getImageData(55, 24, 1, 1).data
    return data[3] === 0 && data2[3] === 0
  } catch {
    return false
  }
}

/**
 * Memecah texture Minecraft menjadi permukaan 3D cuboid untuk base layer dan outer layer 3D.
 */
export async function extractSkinFaces(skinSrc: string): Promise<ExtractedSkinResult | null> {
  if (facesCache.has(skinSrc)) {
    return facesCache.get(skinSrc)!
  }

  return new Promise<ExtractedSkinResult | null>((resolve) => {
    const img = new Image()
    img.crossOrigin = 'anonymous'

    img.onload = () => {
      try {
        const isLegacy = img.naturalHeight * 2 === img.naturalWidth
        const canvas = document.createElement('canvas')
        canvas.width = 64
        canvas.height = 64
        const ctx = canvas.getContext('2d')
        if (!ctx) {
          resolve(null)
          return
        }

        ctx.imageSmoothingEnabled = false
        ctx.drawImage(img, 0, 0, 64, isLegacy ? 32 : 64)

        if (isLegacy) {
          ctx.drawImage(canvas, 40, 16, 16, 16, 32, 48, 16, 16)
          ctx.drawImage(canvas, 0, 16, 16, 16, 16, 48, 16, 16)
        }

        const isSlim = checkSlimModel(canvas)

        const headF = sliceFace(canvas, { x: 8, y: 8, w: 8, h: 8 })
        const headB = sliceFace(canvas, { x: 24, y: 8, w: 8, h: 8 })
        const headR = sliceFace(canvas, { x: 0, y: 8, w: 8, h: 8 })
        const headL = sliceFace(canvas, { x: 16, y: 8, w: 8, h: 8 })
        const headT = sliceFace(canvas, { x: 8, y: 0, w: 8, h: 8 })
        const headBt = sliceFace(canvas, { x: 16, y: 0, w: 8, h: 8 })

        const torsoF = sliceFace(canvas, { x: 20, y: 20, w: 8, h: 12 })
        const torsoB = sliceFace(canvas, { x: 32, y: 20, w: 8, h: 12 })
        const torsoR = sliceFace(canvas, { x: 16, y: 20, w: 4, h: 12 })
        const torsoL = sliceFace(canvas, { x: 28, y: 20, w: 4, h: 12 })
        const torsoT = sliceFace(canvas, { x: 20, y: 16, w: 8, h: 4 })
        const torsoBt = sliceFace(canvas, { x: 28, y: 16, w: 8, h: 4 })

        const armRW = isSlim ? 3 : 4
        const armRF = sliceFace(canvas, { x: 44, y: 20, w: armRW, h: 12 })
        const armRB = sliceFace(canvas, { x: isSlim ? 51 : 52, y: 20, w: armRW, h: 12 })
        const armRR = sliceFace(canvas, { x: 40, y: 20, w: 4, h: 12 })
        const armRL = sliceFace(canvas, { x: isSlim ? 47 : 48, y: 20, w: 4, h: 12 })
        const armRT = sliceFace(canvas, { x: 44, y: 16, w: armRW, h: 4 })
        const armRBt = sliceFace(canvas, { x: isSlim ? 47 : 48, y: 16, w: armRW, h: 4 })

        const armLF = sliceFace(canvas, { x: 36, y: 52, w: armRW, h: 12 })
        const armLB = sliceFace(canvas, { x: isSlim ? 43 : 44, y: 52, w: armRW, h: 12 })
        const armLR = sliceFace(canvas, { x: 32, y: 52, w: 4, h: 12 })
        const armLL = sliceFace(canvas, { x: isSlim ? 39 : 40, y: 52, w: 4, h: 12 })
        const armLT = sliceFace(canvas, { x: 36, y: 48, w: armRW, h: 4 })
        const armLBt = sliceFace(canvas, { x: isSlim ? 39 : 40, y: 48, w: armRW, h: 4 })

        const legRF = sliceFace(canvas, { x: 4, y: 20, w: 4, h: 12 })
        const legRB = sliceFace(canvas, { x: 12, y: 20, w: 4, h: 12 })
        const legRR = sliceFace(canvas, { x: 0, y: 20, w: 4, h: 12 })
        const legRL = sliceFace(canvas, { x: 8, y: 20, w: 4, h: 12 })
        const legRT = sliceFace(canvas, { x: 4, y: 16, w: 4, h: 4 })
        const legRBt = sliceFace(canvas, { x: 8, y: 16, w: 4, h: 4 })

        const legLF = sliceFace(canvas, { x: 20, y: 52, w: 4, h: 12 })
        const legLB = sliceFace(canvas, { x: 28, y: 52, w: 4, h: 12 })
        const legLR = sliceFace(canvas, { x: 16, y: 52, w: 4, h: 12 })
        const legLL = sliceFace(canvas, { x: 24, y: 52, w: 4, h: 12 })
        const legLT = sliceFace(canvas, { x: 20, y: 48, w: 4, h: 4 })
        const legLBt = sliceFace(canvas, { x: 24, y: 48, w: 4, h: 4 })

        const baseFaces: SkinFaces = {
          headFront: headF.url,
          headBack: headB.url,
          headRight: headR.url,
          headLeft: headL.url,
          headTop: headT.url,
          headBottom: headBt.url,

          torsoFront: torsoF.url,
          torsoBack: torsoB.url,
          torsoRight: torsoR.url,
          torsoLeft: torsoL.url,
          torsoTop: torsoT.url,
          torsoBottom: torsoBt.url,

          armRightFront: armRF.url,
          armRightBack: armRB.url,
          armRightRight: armRR.url,
          armRightLeft: armRL.url,
          armRightTop: armRT.url,
          armRightBottom: armRBt.url,

          armLeftFront: armLF.url,
          armLeftBack: armLB.url,
          armLeftRight: armLR.url,
          armLeftLeft: armLL.url,
          armLeftTop: armLT.url,
          armLeftBottom: armLBt.url,

          legRightFront: legRF.url,
          legRightBack: legRB.url,
          legRightRight: legRR.url,
          legRightLeft: legRL.url,
          legRightTop: legRT.url,
          legRightBottom: legRBt.url,

          legLeftFront: legLF.url,
          legLeftBack: legLB.url,
          legLeftRight: legLR.url,
          legLeftLeft: legLL.url,
          legLeftTop: legLT.url,
          legLeftBottom: legLBt.url,
        }

        const headOF = sliceFace(canvas, { x: 40, y: 8, w: 8, h: 8 }, true)
        const headOB = sliceFace(canvas, { x: 56, y: 8, w: 8, h: 8 }, true)
        const headOR = sliceFace(canvas, { x: 32, y: 8, w: 8, h: 8 }, true)
        const headOL = sliceFace(canvas, { x: 48, y: 8, w: 8, h: 8 }, true)
        const headOT = sliceFace(canvas, { x: 40, y: 0, w: 8, h: 8 }, true)
        const headOBt = sliceFace(canvas, { x: 48, y: 0, w: 8, h: 8 }, true)

        const torsoOF = sliceFace(canvas, { x: 20, y: 36, w: 8, h: 12 }, true)
        const torsoOB = sliceFace(canvas, { x: 32, y: 36, w: 8, h: 12 }, true)
        const torsoOR = sliceFace(canvas, { x: 16, y: 36, w: 4, h: 12 }, true)
        const torsoOL = sliceFace(canvas, { x: 28, y: 36, w: 4, h: 12 }, true)
        const torsoOT = sliceFace(canvas, { x: 20, y: 32, w: 8, h: 4 }, true)
        const torsoOBt = sliceFace(canvas, { x: 28, y: 32, w: 8, h: 4 }, true)

        const armROF = sliceFace(canvas, { x: 44, y: 36, w: armRW, h: 12 }, true)
        const armROB = sliceFace(canvas, { x: isSlim ? 51 : 52, y: 36, w: armRW, h: 12 }, true)
        const armROR = sliceFace(canvas, { x: 40, y: 36, w: 4, h: 12 }, true)
        const armROL = sliceFace(canvas, { x: isSlim ? 47 : 48, y: 36, w: 4, h: 12 }, true)
        const armROT = sliceFace(canvas, { x: 44, y: 32, w: armRW, h: 4 }, true)
        const armROBt = sliceFace(canvas, { x: isSlim ? 47 : 48, y: 32, w: armRW, h: 4 }, true)

        const armLOF = sliceFace(canvas, { x: 52, y: 52, w: armRW, h: 12 }, true)
        const armLOB = sliceFace(canvas, { x: isSlim ? 59 : 60, y: 52, w: armRW, h: 12 }, true)
        const armLOR = sliceFace(canvas, { x: 48, y: 52, w: 4, h: 12 }, true)
        const armLOL = sliceFace(canvas, { x: isSlim ? 55 : 56, y: 52, w: 4, h: 12 }, true)
        const armLOT = sliceFace(canvas, { x: 52, y: 48, w: armRW, h: 4 }, true)
        const armLOBt = sliceFace(canvas, { x: isSlim ? 55 : 56, y: 48, w: armRW, h: 4 }, true)

        const legROF = sliceFace(canvas, { x: 4, y: 36, w: 4, h: 12 }, true)
        const legROB = sliceFace(canvas, { x: 12, y: 36, w: 4, h: 12 }, true)
        const legROR = sliceFace(canvas, { x: 0, y: 36, w: 4, h: 12 }, true)
        const legROL = sliceFace(canvas, { x: 8, y: 36, w: 4, h: 12 }, true)
        const legROT = sliceFace(canvas, { x: 4, y: 32, w: 4, h: 4 }, true)
        const legROBt = sliceFace(canvas, { x: 8, y: 32, w: 4, h: 4 }, true)

        const legLOF = sliceFace(canvas, { x: 4, y: 52, w: 4, h: 12 }, true)
        const legLOB = sliceFace(canvas, { x: 12, y: 52, w: 4, h: 12 }, true)
        const legLOR = sliceFace(canvas, { x: 0, y: 52, w: 4, h: 12 }, true)
        const legLOL = sliceFace(canvas, { x: 8, y: 52, w: 4, h: 12 }, true)
        const legLOT = sliceFace(canvas, { x: 4, y: 48, w: 4, h: 4 }, true)
        const legLOBt = sliceFace(canvas, { x: 8, y: 48, w: 4, h: 4 }, true)

        const overlayFaces: SkinFaces = {
          headFront: headOF.url,
          headBack: headOB.url,
          headRight: headOR.url,
          headLeft: headOL.url,
          headTop: headOT.url,
          headBottom: headOBt.url,

          torsoFront: torsoOF.url,
          torsoBack: torsoOB.url,
          torsoRight: torsoOR.url,
          torsoLeft: torsoOL.url,
          torsoTop: torsoOT.url,
          torsoBottom: torsoOBt.url,

          armRightFront: armROF.url,
          armRightBack: armROB.url,
          armRightRight: armROR.url,
          armRightLeft: armROL.url,
          armRightTop: armROT.url,
          armRightBottom: armROBt.url,

          armLeftFront: armLOF.url,
          armLeftBack: armLOB.url,
          armLeftRight: armLOR.url,
          armLeftLeft: armLOL.url,
          armLeftTop: armLOT.url,
          armLeftBottom: armLOBt.url,

          legRightFront: legROF.url,
          legRightBack: legROB.url,
          legRightRight: legROR.url,
          legRightLeft: legROL.url,
          legRightTop: legROT.url,
          legRightBottom: legROBt.url,

          legLeftFront: legLOF.url,
          legLeftBack: legLOB.url,
          legLeftRight: legLOR.url,
          legLeftLeft: legLOL.url,
          legLeftTop: legLOT.url,
          legLeftBottom: legLOBt.url,
        }

        const hasOverlay: OverlayVisibility = {
          head:
            headOF.hasPixels ||
            headOB.hasPixels ||
            headOR.hasPixels ||
            headOL.hasPixels ||
            headOT.hasPixels ||
            headOBt.hasPixels,
          torso:
            torsoOF.hasPixels ||
            torsoOB.hasPixels ||
            torsoOR.hasPixels ||
            torsoOL.hasPixels ||
            torsoOT.hasPixels ||
            torsoOBt.hasPixels,
          armRight:
            armROF.hasPixels ||
            armROB.hasPixels ||
            armROR.hasPixels ||
            armROL.hasPixels ||
            armROT.hasPixels ||
            armROBt.hasPixels,
          armLeft:
            armLOF.hasPixels ||
            armLOB.hasPixels ||
            armLOR.hasPixels ||
            armLOL.hasPixels ||
            armLOT.hasPixels ||
            armLOBt.hasPixels,
          legRight:
            legROF.hasPixels ||
            legROB.hasPixels ||
            legROR.hasPixels ||
            legROL.hasPixels ||
            legROT.hasPixels ||
            legROBt.hasPixels,
          legLeft:
            legLOF.hasPixels ||
            legLOB.hasPixels ||
            legLOR.hasPixels ||
            legLOL.hasPixels ||
            legLOT.hasPixels ||
            legLOBt.hasPixels,
        }

        const result: ExtractedSkinResult = {
          faces: baseFaces,
          baseFaces,
          overlayFaces,
          hasOverlay,
          isSlim,
        }
        setFacesCache(skinSrc, result)
        resolve(result)
      } catch {
        resolve(null)
      }
    }

    img.onerror = () => {
      resolve(null)
    }

    img.src = skinSrc
  })
}

export interface CapeFaces {
  front: string
  back: string
  left: string
  right: string
  top: string
  bottom: string
}

const capeCache = new Map<string, CapeFaces>()

export async function extractCapeFaces(capeSrc: string | undefined | null): Promise<CapeFaces | null> {
  if (!capeSrc || capeSrc.trim() === '') return null
  if (capeCache.has(capeSrc)) {
    return capeCache.get(capeSrc)!
  }

  return new Promise<CapeFaces | null>((resolve) => {
    const img = new Image()
    img.crossOrigin = 'anonymous'

    img.onload = () => {
      try {
        const canvas = document.createElement('canvas')
        canvas.width = 64
        canvas.height = 32
        const ctx = canvas.getContext('2d')
        if (!ctx) {
          resolve(null)
          return
        }

        ctx.imageSmoothingEnabled = false
        ctx.drawImage(img, 0, 0, 64, 32)

        const top = sliceFace(canvas, { x: 1, y: 0, w: 10, h: 1 })
        const bottom = sliceFace(canvas, { x: 11, y: 0, w: 10, h: 1 })
        const right = sliceFace(canvas, { x: 0, y: 1, w: 1, h: 16 })
        const back = sliceFace(canvas, { x: 1, y: 1, w: 10, h: 16 })
        const left = sliceFace(canvas, { x: 11, y: 1, w: 1, h: 16 })
        const front = sliceFace(canvas, { x: 12, y: 1, w: 10, h: 16 })

        const res: CapeFaces = {
          top: top.url,
          bottom: bottom.url,
          left: left.url,
          right: right.url,
          front: front.url,
          back: back.url,
        }
        if (capeCache.size >= 10) {
          const oldest = capeCache.keys().next().value
          if (oldest) capeCache.delete(oldest)
        }
        capeCache.set(capeSrc, res)
        resolve(res)
      } catch {
        resolve(null)
      }
    }

    img.onerror = () => {
      resolve(null)
    }

    img.src = capeSrc
  })
}
