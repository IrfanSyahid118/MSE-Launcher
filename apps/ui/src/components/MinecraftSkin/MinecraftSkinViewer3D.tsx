import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import * as THREE from 'three'
import { useI18n } from '../../i18n'
import { extractCapeFaces, extractSkinFaces, getSteveSkinDataUrl } from './skin-textures'
import type { CapeFaces, ExtractedSkinResult } from './skin-textures'
import { SkinFallback } from './SkinFallback'
import styles from './MinecraftSkinViewer3D.module.css'

export interface MinecraftSkinViewer3DProps {
  skinUrl?: string | undefined
  capeUrl?: string | undefined
  isSlim?: boolean | undefined
  interactive?: boolean | undefined
  className?: string | undefined
  autoRotateSpeed?: number | undefined
  /** Tanpa kotak latar, bingkai, dan bayangan (mis. di halaman Skin selector). */
  bare?: boolean | undefined
  /** Faktor jarak kamera; nilai > 1 memperkecil model di dalam bingkai. */
  zoom?: number | undefined
  /** Arahkan kamera langsung ke bagian belakang karakter (untuk melihat jubah). */
  viewBack?: boolean | undefined
}

interface BodyPartMeshes {
  headGroup: THREE.Group
  torsoGroup: THREE.Group
  leftArmGroup: THREE.Group
  rightArmGroup: THREE.Group
  leftLegGroup: THREE.Group
  rightLegGroup: THREE.Group
  modelRoot: THREE.Group
}

const EMPTY_MATERIAL = new THREE.MeshBasicMaterial({
  transparent: true,
  opacity: 0,
  depthWrite: false,
})

function disposeModel(root: THREE.Object3D): void {
  root.traverse((node) => {
    if (!(node instanceof THREE.Mesh)) return
    node.geometry.dispose()
    const materials = Array.isArray(node.material) ? node.material : [node.material]
    for (const material of materials) {
      if (material === EMPTY_MATERIAL) continue
      for (const value of Object.values(material)) {
        if (value instanceof THREE.Texture) value.dispose()
      }
      material.dispose()
    }
  })
}

/**
 * Creates 6 materials from face image data URLs for a BoxGeometry.
 * Face order for Three.js BoxGeometry:
 * [0: +X (Right), 1: -X (Left), 2: +Y (Top), 3: -Y (Bottom), 4: +Z (Front), 5: -Z (Back)]
 */
function createBoxMaterials(
  faces: {
    front?: string
    back?: string
    left?: string
    right?: string
    top?: string
    bottom?: string
  },
  isOverlay = false,
): THREE.Material[] {
  const textureLoader = new THREE.TextureLoader()

  const makeMat = (url?: string): THREE.Material => {
    if (!url) {
      return EMPTY_MATERIAL
    }

    const texture = textureLoader.load(url)
    texture.magFilter = THREE.NearestFilter
    texture.minFilter = THREE.NearestFilter
    texture.generateMipmaps = false
    texture.colorSpace = THREE.SRGBColorSpace

    if (isOverlay) {
      return new THREE.MeshBasicMaterial({
        map: texture,
        transparent: true,
        alphaTest: 0.08,
        side: THREE.DoubleSide,
      })
    }

    return new THREE.MeshBasicMaterial({
      map: texture,
    })
  }

  // BoxGeometry face order: +X (left), -X (right), +Y (top), -Y (bottom), +Z (front), -Z (back).
  return [
    makeMat(faces.left),
    makeMat(faces.right),
    makeMat(faces.top),
    makeMat(faces.bottom),
    makeMat(faces.front),
    makeMat(faces.back),
  ]
}

export function MinecraftSkinViewer3D({
  skinUrl,
  capeUrl,
  isSlim,
  interactive = true,
  className,
  autoRotateSpeed = 0.5,
  bare = false,
  zoom = 1,
  viewBack = false,
}: MinecraftSkinViewer3DProps): ReactNode {
  const { t } = useI18n()
  const containerRef = useRef<HTMLDivElement>(null)
  // Create the canvas imperatively so React StrictMode remounts do not reuse a lost WebGL context.
  const canvasHostRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement | null>(null)

  const [skinData, setSkinData] = useState<ExtractedSkinResult | null>(null)
  const [capeData, setCapeData] = useState<CapeFaces | null>(null)
  const [detectedSlim, setDetectedSlim] = useState(false)
  const [isDragging, setIsDragging] = useState(false)
  const [isRenderable, setIsRenderable] = useState(true)
  const [webglFailed, setWebglFailed] = useState(false)

  const sceneRef = useRef<THREE.Scene | null>(null)
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null)
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null)
  const bodyPartsRef = useRef<BodyPartMeshes | null>(null)

  const currentYawRef = useRef<number>(-0.4)
  const currentPitchRef = useRef<number>(-0.08)
  const targetYawRef = useRef<number>(-0.4)
  const targetPitchRef = useRef<number>(-0.08)
  const cameraDistRef = useRef<number>(52)
  const targetDistRef = useRef<number>(52)

  useEffect(() => {
    const dist = 52 * zoom
    cameraDistRef.current = dist
    targetDistRef.current = dist
  }, [zoom])

  useEffect(() => {
    if (viewBack) {
      targetYawRef.current = Math.PI - 0.35
    } else {
      targetYawRef.current = -0.4
    }
  }, [viewBack])

  const isDraggingRef = useRef(false)
  const lastMousePosRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 })
  const animFrameIdRef = useRef<number | null>(null)
  const lostTimerRef = useRef<number | null>(null)
  const clockRef = useRef(new THREE.Clock())
  const contextLostRef = useRef(false)

  const resolvedSkinUrl = skinUrl && skinUrl.trim() !== '' ? skinUrl : getSteveSkinDataUrl()

  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const updateVisibility = (): void => {
      setIsRenderable(document.visibilityState === 'visible')
    }
    const observer = new IntersectionObserver((entries) => {
      const entry = entries[0]
      if (entry) {
        setIsRenderable(entry.isIntersecting && document.visibilityState === 'visible')
      }
    }, { threshold: 0.01 })

    observer.observe(container)
    document.addEventListener('visibilitychange', updateVisibility)
    updateVisibility()

    return () => {
      observer.disconnect()
      document.removeEventListener('visibilitychange', updateVisibility)
    }
  }, [])

  useEffect(() => {
    let active = true
    void extractSkinFaces(resolvedSkinUrl).then((res) => {
      if (active) {
        if (res) {
          setSkinData(res)
          setDetectedSlim(res.isSlim)
        } else {
          void extractSkinFaces(getSteveSkinDataUrl()).then((fallbackRes) => {
            if (active && fallbackRes) {
              setSkinData(fallbackRes)
              setDetectedSlim(fallbackRes.isSlim)
            }
          })
        }
      }
    })
    return () => {
      active = false
    }
  }, [resolvedSkinUrl])

  useEffect(() => {
    let active = true
    if (!capeUrl || capeUrl.trim() === '') {
      setCapeData(null)
      return
    }
    void extractCapeFaces(capeUrl).then((res) => {
      if (active) {
        setCapeData(res)
      }
    })
    return () => {
      active = false
    }
  }, [capeUrl])

  useEffect(() => {
    const container = containerRef.current
    const host = canvasHostRef.current
    if (!container || !host) return

    const canvas = document.createElement('canvas')
    canvas.className = styles.canvas ?? ''
    host.appendChild(canvas)
    canvasRef.current = canvas

    const width = container.clientWidth || 300
    const height = container.clientHeight || 400

    const scene = new THREE.Scene()
    sceneRef.current = scene

    const camera = new THREE.PerspectiveCamera(40, width / height, 0.1, 1000)
    camera.position.set(0, 16, 52)
    cameraRef.current = camera

    const renderer = (() => {
      try {
        return new THREE.WebGLRenderer({
          canvas,
          alpha: true,
          antialias: false,
          // Avoid powerPreference: 'low-power', which forces software rendering on some Windows GPUs.
          failIfMajorPerformanceCaveat: false,
        })
      } catch {
        return null
      }
    })()

    if (!renderer) {
      sceneRef.current = null
      cameraRef.current = null
      canvas.remove()
      canvasRef.current = null
      setWebglFailed(true)
      return
    }

    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.5))
    renderer.setSize(width, height)
    renderer.outputColorSpace = THREE.SRGBColorSpace
    rendererRef.current = renderer

    const handleContextLost = (event: Event): void => {
      // Allow Windows GPU drivers a short window to recover before switching to 2D fallback.
      event.preventDefault()
      contextLostRef.current = true
      if (lostTimerRef.current !== null) {
        window.clearTimeout(lostTimerRef.current)
      }
      lostTimerRef.current = window.setTimeout(() => {
        lostTimerRef.current = null
        if (!contextLostRef.current) return
        if (animFrameIdRef.current !== null) {
          cancelAnimationFrame(animFrameIdRef.current)
          animFrameIdRef.current = null
        }
        if (bodyPartsRef.current) {
          scene.remove(bodyPartsRef.current.modelRoot)
          disposeModel(bodyPartsRef.current.modelRoot)
          bodyPartsRef.current = null
        }
        renderer.dispose()
        rendererRef.current = null
        cameraRef.current = null
        sceneRef.current = null
        // Detach the dead canvas so the underlying 2D preview becomes visible.
        canvas.remove()
        canvasRef.current = null
        setWebglFailed(true)
      }, 1500)
    }
    const handleContextRestored = (): void => {
      contextLostRef.current = false
      if (lostTimerRef.current !== null) {
        window.clearTimeout(lostTimerRef.current)
        lostTimerRef.current = null
      }
      if (rendererRef.current !== null) {
        setWebglFailed(false)
      }
    }
    canvas.addEventListener('webglcontextlost', handleContextLost)
    canvas.addEventListener('webglcontextrestored', handleContextRestored)

    const handleResize = (): void => {
      if (!container || !renderer || !camera) return
      const w = container.clientWidth
      const h = container.clientHeight
      if (w === 0 || h === 0) return
      camera.aspect = w / h
      camera.updateProjectionMatrix()
      renderer.setSize(w, h)
    }

    const resizeObserver = new ResizeObserver(handleResize)
    resizeObserver.observe(container)

    return () => {
      resizeObserver.disconnect()
      canvas.removeEventListener('webglcontextlost', handleContextLost)
      canvas.removeEventListener('webglcontextrestored', handleContextRestored)
      if (lostTimerRef.current !== null) {
        window.clearTimeout(lostTimerRef.current)
        lostTimerRef.current = null
      }
      if (animFrameIdRef.current) {
        cancelAnimationFrame(animFrameIdRef.current)
      }
      if (bodyPartsRef.current) {
        scene.remove(bodyPartsRef.current.modelRoot)
        bodyPartsRef.current = null
      }
      renderer.dispose()
      renderer.forceContextLoss()
      canvas.remove()
      canvasRef.current = null
      rendererRef.current = null
      cameraRef.current = null
      sceneRef.current = null
      contextLostRef.current = false
    }
  }, [])

  useEffect(() => {
    const scene = sceneRef.current
    if (!scene || !skinData) return

    const effectiveSlim = isSlim !== undefined ? isSlim : detectedSlim
    const armW = effectiveSlim ? 3 : 4
    const { baseFaces, overlayFaces, hasOverlay } = skinData

    const modelRoot = new THREE.Group()
    modelRoot.position.set(0, 0, 0)

    const headGroup = new THREE.Group()
    headGroup.position.set(0, 24, 0)

    const headGeo = new THREE.BoxGeometry(8, 8, 8)
    const headMaterials = createBoxMaterials({
      front: baseFaces.headFront,
      back: baseFaces.headBack,
      left: baseFaces.headLeft,
      right: baseFaces.headRight,
      top: baseFaces.headTop,
      bottom: baseFaces.headBottom,
    })
    const headMesh = new THREE.Mesh(headGeo, headMaterials)
    headMesh.position.set(0, 4, 0)
    headGroup.add(headMesh)

    if (hasOverlay.head) {
      const hatGeo = new THREE.BoxGeometry(9, 9, 9)
      const hatMaterials = createBoxMaterials(
        {
          front: overlayFaces.headFront,
          back: overlayFaces.headBack,
          left: overlayFaces.headLeft,
          right: overlayFaces.headRight,
          top: overlayFaces.headTop,
          bottom: overlayFaces.headBottom,
        },
        true,
      )
      const hatMesh = new THREE.Mesh(hatGeo, hatMaterials)
      hatMesh.position.set(0, 4, 0)
      headGroup.add(hatMesh)
    }
    modelRoot.add(headGroup)

    const torsoGroup = new THREE.Group()
    torsoGroup.position.set(0, 12, 0)

    const torsoGeo = new THREE.BoxGeometry(8, 12, 4)
    const torsoMaterials = createBoxMaterials({
      front: baseFaces.torsoFront,
      back: baseFaces.torsoBack,
      left: baseFaces.torsoLeft,
      right: baseFaces.torsoRight,
      top: baseFaces.torsoTop,
      bottom: baseFaces.torsoBottom,
    })
    const torsoMesh = new THREE.Mesh(torsoGeo, torsoMaterials)
    torsoMesh.position.set(0, 6, 0)
    torsoGroup.add(torsoMesh)

    if (hasOverlay.torso) {
      const jacketGeo = new THREE.BoxGeometry(8.6, 12.6, 4.6)
      const jacketMaterials = createBoxMaterials(
        {
          front: overlayFaces.torsoFront,
          back: overlayFaces.torsoBack,
          left: overlayFaces.torsoLeft,
          right: overlayFaces.torsoRight,
          top: overlayFaces.torsoTop,
          bottom: overlayFaces.torsoBottom,
        },
        true,
      )
      const jacketMesh = new THREE.Mesh(jacketGeo, jacketMaterials)
      jacketMesh.position.set(0, 6, 0)
      torsoGroup.add(jacketMesh)
    }

    if (capeData) {
      const capeMaterials = createBoxMaterials(capeData)
      const capeGeo = new THREE.BoxGeometry(10, 16, 1)
      // Pivot at the top-front edge so the cape swings back away from the torso.
      capeGeo.translate(0, -8, -0.5)
      const capeMesh = new THREE.Mesh(capeGeo, capeMaterials)
      capeMesh.position.set(0, 12, hasOverlay.torso ? -2.35 : -2.1)
      capeMesh.rotation.x = 0.16
      torsoGroup.add(capeMesh)
    }

    modelRoot.add(torsoGroup)

    const rightArmGroup = new THREE.Group()
    // 0.02 gap prevents z-fighting against the torso side faces.
    const armClearance = 0.02
    const armRPivotX = -(4 + armW / 2 + armClearance)
    rightArmGroup.position.set(armRPivotX, 22, 0)

    const armRGeo = new THREE.BoxGeometry(armW, 12, 4)
    const armRMaterials = createBoxMaterials({
      front: baseFaces.armRightFront,
      back: baseFaces.armRightBack,
      left: baseFaces.armRightLeft,
      right: baseFaces.armRightRight,
      top: baseFaces.armRightTop,
      bottom: baseFaces.armRightBottom,
    })
    const armRMesh = new THREE.Mesh(armRGeo, armRMaterials)
    armRMesh.position.set(0, -5, 0)
    rightArmGroup.add(armRMesh)

    if (hasOverlay.armRight) {
      const sleeveRGeo = new THREE.BoxGeometry(armW + 0.6, 12.6, 4.6)
      const sleeveRMaterials = createBoxMaterials(
        {
          front: overlayFaces.armRightFront,
          back: overlayFaces.armRightBack,
          left: overlayFaces.armRightLeft,
          right: overlayFaces.armRightRight,
          top: overlayFaces.armRightTop,
          bottom: overlayFaces.armRightBottom,
        },
        true,
      )
      const sleeveRMesh = new THREE.Mesh(sleeveRGeo, sleeveRMaterials)
      // Offset outward so sleeve overlay does not clip through the torso.
      sleeveRMesh.position.set(-0.3, -5, 0)
      rightArmGroup.add(sleeveRMesh)
    }
    modelRoot.add(rightArmGroup)

    const leftArmGroup = new THREE.Group()
    const armLPivotX = 4 + armW / 2 + armClearance
    leftArmGroup.position.set(armLPivotX, 22, 0)

    const armLGeo = new THREE.BoxGeometry(armW, 12, 4)
    const armLMaterials = createBoxMaterials({
      front: baseFaces.armLeftFront,
      back: baseFaces.armLeftBack,
      left: baseFaces.armLeftLeft,
      right: baseFaces.armLeftRight,
      top: baseFaces.armLeftTop,
      bottom: baseFaces.armLeftBottom,
    })
    const armLMesh = new THREE.Mesh(armLGeo, armLMaterials)
    armLMesh.position.set(0, -5, 0)
    leftArmGroup.add(armLMesh)

    if (hasOverlay.armLeft) {
      const sleeveLGeo = new THREE.BoxGeometry(armW + 0.6, 12.6, 4.6)
      const sleeveLMaterials = createBoxMaterials(
        {
          front: overlayFaces.armLeftFront,
          back: overlayFaces.armLeftBack,
          left: overlayFaces.armLeftLeft,
          right: overlayFaces.armLeftRight,
          top: overlayFaces.armLeftTop,
          bottom: overlayFaces.armLeftBottom,
        },
        true,
      )
      const sleeveLMesh = new THREE.Mesh(sleeveLGeo, sleeveLMaterials)
      sleeveLMesh.position.set(0.3, -5, 0)
      leftArmGroup.add(sleeveLMesh)
    }
    modelRoot.add(leftArmGroup)

    const rightLegGroup = new THREE.Group()
    rightLegGroup.position.set(-2 - armClearance, 12, 0)

    const legRGeo = new THREE.BoxGeometry(4, 12, 4)
    const legRMaterials = createBoxMaterials({
      front: baseFaces.legRightFront,
      back: baseFaces.legRightBack,
      left: baseFaces.legRightLeft,
      right: baseFaces.legRightRight,
      top: baseFaces.legRightTop,
      bottom: baseFaces.legRightBottom,
    })
    const legRMesh = new THREE.Mesh(legRGeo, legRMaterials)
    legRMesh.position.set(0, -6, 0)
    rightLegGroup.add(legRMesh)

    if (hasOverlay.legRight) {
      const pantsRGeo = new THREE.BoxGeometry(4.6, 12.6, 4.6)
      const pantsRMaterials = createBoxMaterials(
        {
          front: overlayFaces.legRightFront,
          back: overlayFaces.legRightBack,
          left: overlayFaces.legRightLeft,
          right: overlayFaces.legRightRight,
          top: overlayFaces.legRightTop,
          bottom: overlayFaces.legRightBottom,
        },
        true,
      )
      const pantsRMesh = new THREE.Mesh(pantsRGeo, pantsRMaterials)
      pantsRMesh.position.set(-0.3, -6, 0)
      rightLegGroup.add(pantsRMesh)
    }
    modelRoot.add(rightLegGroup)

    const leftLegGroup = new THREE.Group()
    leftLegGroup.position.set(2 + armClearance, 12, 0)

    const legLGeo = new THREE.BoxGeometry(4, 12, 4)
    const legLMaterials = createBoxMaterials({
      front: baseFaces.legLeftFront,
      back: baseFaces.legLeftBack,
      left: baseFaces.legLeftLeft,
      right: baseFaces.legLeftRight,
      top: baseFaces.legLeftTop,
      bottom: baseFaces.legLeftBottom,
    })
    const legLMesh = new THREE.Mesh(legLGeo, legLMaterials)
    legLMesh.position.set(0, -6, 0)
    leftLegGroup.add(legLMesh)

    if (hasOverlay.legLeft) {
      const pantsLGeo = new THREE.BoxGeometry(4.6, 12.6, 4.6)
      const pantsLMaterials = createBoxMaterials(
        {
          front: overlayFaces.legLeftFront,
          back: overlayFaces.legLeftBack,
          left: overlayFaces.legLeftLeft,
          right: overlayFaces.legLeftRight,
          top: overlayFaces.legLeftTop,
          bottom: overlayFaces.legLeftBottom,
        },
        true,
      )
      const pantsLMesh = new THREE.Mesh(pantsLGeo, pantsLMaterials)
      pantsLMesh.position.set(0.3, -6, 0)
      leftLegGroup.add(pantsLMesh)
    }
    modelRoot.add(leftLegGroup)

    scene.add(modelRoot)
    bodyPartsRef.current = {
      headGroup,
      torsoGroup,
      leftArmGroup,
      rightArmGroup,
      leftLegGroup,
      rightLegGroup,
      modelRoot,
    }

    return () => {
      scene.remove(modelRoot)
      disposeModel(modelRoot)
      if (bodyPartsRef.current?.modelRoot === modelRoot) {
        bodyPartsRef.current = null
      }
    }
  }, [skinData, isSlim, detectedSlim, capeData])

  useEffect(() => {
    if (!isRenderable) return
    let animId: number | null = null

    const animate = (): void => {
      if (document.hidden) {
        animId = null
        return
      }
      animId = requestAnimationFrame(animate)
      animFrameIdRef.current = animId

      const elapsedTime = clockRef.current.getElapsedTime()
      const parts = bodyPartsRef.current
      const scene = sceneRef.current
      const camera = cameraRef.current
      const renderer = rendererRef.current

      currentYawRef.current += (targetYawRef.current - currentYawRef.current) * 0.12
      currentPitchRef.current = 0
      targetPitchRef.current = 0
      cameraDistRef.current += (targetDistRef.current - cameraDistRef.current) * 0.14

      if (camera) {
        const yaw = currentYawRef.current
        const dist = cameraDistRef.current

        const cx = Math.sin(yaw) * dist
        const cy = 16
        const cz = Math.cos(yaw) * dist

        camera.position.set(cx, cy, cz)
        camera.lookAt(0, 16, 0)
      }

      if (parts) {
        if (!isDraggingRef.current) {
          targetYawRef.current += 0.0025 * autoRotateSpeed

          const breath = Math.sin(elapsedTime * 2.2) * 0.18
          parts.modelRoot.position.y = breath

          parts.headGroup.rotation.y = Math.sin(elapsedTime * 1.0) * 0.05
          parts.headGroup.rotation.x = Math.sin(elapsedTime * 1.4) * 0.03

          parts.leftArmGroup.rotation.x = Math.sin(elapsedTime * 1.6) * 0.06
          parts.rightArmGroup.rotation.x = -Math.sin(elapsedTime * 1.6) * 0.06
          parts.leftArmGroup.rotation.z = Math.abs(Math.sin(elapsedTime * 2.2)) * 0.02
          parts.rightArmGroup.rotation.z = -Math.abs(Math.sin(elapsedTime * 2.2)) * 0.02
        } else {
          parts.modelRoot.position.y *= 0.9
          parts.headGroup.rotation.x *= 0.9
          parts.headGroup.rotation.y *= 0.9
          parts.leftArmGroup.rotation.x *= 0.9
          parts.rightArmGroup.rotation.x *= 0.9
          parts.leftArmGroup.rotation.z *= 0.9
          parts.rightArmGroup.rotation.z *= 0.9
        }
      }

      if (renderer && scene && camera) {
        renderer.render(scene, camera)
      }
    }

    animId = requestAnimationFrame(animate)

    const onVisibilityChange = (): void => {
      if (!document.hidden && animId === null) {
        animId = requestAnimationFrame(animate)
        animFrameIdRef.current = animId
      }
    }
    document.addEventListener('visibilitychange', onVisibilityChange)

    return () => {
      document.removeEventListener('visibilitychange', onVisibilityChange)
      if (animId !== null) {
        cancelAnimationFrame(animId)
      }
    }
  }, [autoRotateSpeed, isRenderable])

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>): void => {
    if (!interactive) return
    isDraggingRef.current = true
    setIsDragging(true)
    lastMousePosRef.current = { x: e.clientX, y: e.clientY }
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  }

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>): void => {
    if (!isDraggingRef.current || !interactive) return

    const dx = e.clientX - lastMousePosRef.current.x
    lastMousePosRef.current = { x: e.clientX, y: e.clientY }

    targetYawRef.current += dx * 0.012
    targetPitchRef.current = 0
  }

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>): void => {
    isDraggingRef.current = false
    setIsDragging(false)
    try {
      ;(e.target as HTMLElement).releasePointerCapture(e.pointerId)
    } catch {
    }
  }

  useEffect(() => {
    const el = containerRef.current
    if (!el || !interactive) return

    const onWheel = (e: WheelEvent): void => {
      e.preventDefault()
      targetDistRef.current = Math.max(
        36,
        Math.min(76, targetDistRef.current + e.deltaY * 0.05),
      )
    }

    el.addEventListener('wheel', onWheel, { passive: false })
    return () => {
      el.removeEventListener('wheel', onWheel)
    }
  }, [interactive])

  return (
    <div
      ref={containerRef}
      className={`${styles.container} ${bare ? styles.bare : ''} ${className ?? ''} ${isDragging ? styles.dragging : ''}`}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
      role="region"
      aria-label={t('skin.preview3dLabel')}
      tabIndex={0}
      title={
        webglFailed
          ? undefined
          : interactive
            ? t('skin.preview3dHint')
            : undefined
      }
    >
      {/*
        Canvas hidup di dalam host ini dan dibuat oleh efek, bukan oleh React.
        Host selalu ada supaya efek punya tempat menempel sejak mount pertama;
        fallback 2D menutupinya saat WebGL tidak tersedia.
      */}
      <div ref={canvasHostRef} className={styles.canvasHost} aria-hidden />
      {webglFailed ? (
        <SkinFallback
          skinUrl={resolvedSkinUrl}
          isSlim={isSlim ?? detectedSlim}
          label={t('skin.preview2dLabel')}
        />
      ) : null}
    </div>
  )
}
