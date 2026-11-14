import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import * as THREE from 'three'
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

  // BoxGeometry face sequence: [+X, -X, +Y, -Y, +Z, -Z]
  // In character space (+Z is front, +X is character's left, -X is character's right):
  return [
    makeMat(faces.left), // +X face (character's left side)
    makeMat(faces.right), // -X face (character's right side)
    makeMat(faces.top), // +Y face (top)
    makeMat(faces.bottom), // -Y face (bottom)
    makeMat(faces.front), // +Z face (front)
    makeMat(faces.back), // -Z face (back)
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
  const containerRef = useRef<HTMLDivElement>(null)
  // Kanvas dibuat oleh efek, bukan oleh React. Konteks WebGL yang sudah
  // di-lose tetap terikat pada elemennya, sehingga memakai ulang elemen yang
  // sama (seperti pada mount kedua StrictMode) membuat three.js menerima
  // konteks mati dan melempar saat membaca `precision`.
  const canvasHostRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement | null>(null)

  const [skinData, setSkinData] = useState<ExtractedSkinResult | null>(null)
  const [capeData, setCapeData] = useState<CapeFaces | null>(null)
  const [detectedSlim, setDetectedSlim] = useState(false)
  const [isDragging, setIsDragging] = useState(false)
  const [isRenderable, setIsRenderable] = useState(true)
  const [webglFailed, setWebglFailed] = useState(false)

  // Camera and scene references
  const sceneRef = useRef<THREE.Scene | null>(null)
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null)
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null)
  const bodyPartsRef = useRef<BodyPartMeshes | null>(null)

  // Interaksi kamera; jarak mengikuti prop zoom saat itu juga.
  const currentYawRef = useRef<number>(-0.4) // ~ -23 deg
  const currentPitchRef = useRef<number>(-0.08) // ~ -5 deg
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

  // 1. Load skin texture and slice into faces
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

  // 1b. Load cape texture and slice into faces
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

  // 2. Initialize Three.js scene, camera, and lights
  useEffect(() => {
    const container = containerRef.current
    const host = canvasHostRef.current
    if (!container || !host) return

    /*
     * Canvas dibuat sendiri, bukan lewat JSX. Canvas yang konteks WebGL-nya
     * sudah dilepas tidak bisa dipakai ulang: `getContext` mengembalikan
     * konteks yang sama dalam keadaan lost, dan three.js melempar saat membaca
     * `precision`. React memakai ulang elemen yang sama pada mount kedua
     * (StrictMode di dev), jadi elemen dari JSX akan mewarisi konteks mati.
     * Canvas baru per mount membuat siklus lepas-pakai selalu bersih.
     */
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
          // `low-power` dapat memaksa driver memakai renderer perangkat lunak
          // di sebagian mesin Windows, jadi biarkan pilihan pada driver.
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
      // Jangan langsung menyerah: driver Windows bisa memulihkan konteks.
      event.preventDefault()
      contextLostRef.current = true
      // Kalau konteks tidak kembali, tampilkan pratinjau 2D supaya tidak kosong.
      lostTimerRef.current = window.setTimeout(() => {
        lostTimerRef.current = null
        if (!contextLostRef.current) return
        // Kanvas yang konteksnya hilang tidak menggambar apa pun; lepaskan
        // supaya pratinjau 2D di bawahnya benar-benar terlihat.
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
      setWebglFailed(false)
    }
    canvas.addEventListener('webglcontextlost', handleContextLost)
    canvas.addEventListener('webglcontextrestored', handleContextRestored)

    // Model memakai MeshBasicMaterial unlit murni tanpa lighting agar warna tekstur piksel skin akurat.

    // Resize observer
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

  // 3. Build or update character mesh when skinData changes
  useEffect(() => {
    const scene = sceneRef.current
    if (!scene || !skinData) return

    const effectiveSlim = isSlim !== undefined ? isSlim : detectedSlim
    const armW = effectiveSlim ? 3 : 4
    const { baseFaces, overlayFaces, hasOverlay } = skinData

    const modelRoot = new THREE.Group()
    modelRoot.position.set(0, 0, 0)

    // ----------------- HEAD GROUP -----------------
    const headGroup = new THREE.Group()
    headGroup.position.set(0, 24, 0) // Pivot at neck

    // Base Head: 8 x 8 x 8
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

    // Outer Hat / Hair Overlay: 9 x 9 x 9
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

    // ----------------- TORSO GROUP -----------------
    const torsoGroup = new THREE.Group()
    torsoGroup.position.set(0, 12, 0) // Pivot at waist

    // Base Torso: 8 x 12 x 4
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

    // Outer Jacket Overlay: 8.6 x 12.6 x 4.6
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

    // Cape: 10 x 16 x 1 (standar Minecraft)
    if (capeData) {
      const capeMaterials = createBoxMaterials(capeData)
      const capeGeo = new THREE.BoxGeometry(10, 16, 1)
      // Pivot di sisi depan-atas cape agar permukaan dalamnya menempel di belakang torso/jacket
      capeGeo.translate(0, -8, -0.5)
      const capeMesh = new THREE.Mesh(capeGeo, capeMaterials)
      capeMesh.position.set(0, 12, hasOverlay.torso ? -2.35 : -2.1)
      // Rotasi +X membuat bagian bawah cape (-Y) mengayun ke belakang (-Z) menjauhi tubuh
      capeMesh.rotation.x = 0.16
      torsoGroup.add(capeMesh)
    }

    modelRoot.add(torsoGroup)

    // ----------------- RIGHT ARM GROUP -----------------
    const rightArmGroup = new THREE.Group()
    // Celah 0.02 supaya sisi lengan tidak berimpit dengan sisi torso (z-fighting).
    const armClearance = 0.02
    const armRPivotX = -(4 + armW / 2 + armClearance)
    rightArmGroup.position.set(armRPivotX, 22, 0) // Pivot at shoulder

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
      // Geser jaket ke sisi luar supaya tidak menembus torso.
      sleeveRMesh.position.set(-0.3, -5, 0)
      rightArmGroup.add(sleeveRMesh)
    }
    modelRoot.add(rightArmGroup)

    // ----------------- LEFT ARM GROUP -----------------
    const leftArmGroup = new THREE.Group()
    const armLPivotX = 4 + armW / 2 + armClearance
    leftArmGroup.position.set(armLPivotX, 22, 0) // Pivot at shoulder

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

    // ----------------- RIGHT LEG GROUP -----------------
    const rightLegGroup = new THREE.Group()
    rightLegGroup.position.set(-2 - armClearance, 12, 0) // Pivot at hip

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

    // ----------------- LEFT LEG GROUP -----------------
    const leftLegGroup = new THREE.Group()
    leftLegGroup.position.set(2 + armClearance, 12, 0) // Pivot at hip

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

  // 4. Main Animation & Render Loop
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

      // Interpolasi yaw dan jarak kamera; pitch dikunci ke 0 (hanya rotasi 360° kiri-kanan)
      currentYawRef.current += (targetYawRef.current - currentYawRef.current) * 0.12
      currentPitchRef.current = 0
      targetPitchRef.current = 0
      cameraDistRef.current += (targetDistRef.current - cameraDistRef.current) * 0.14

      // Orbit kamera horizontal mengitari titik pusat (0, 16, 0)
      if (camera) {
        const yaw = currentYawRef.current
        const dist = cameraDistRef.current

        const cx = Math.sin(yaw) * dist
        const cy = 16
        const cz = Math.cos(yaw) * dist

        camera.position.set(cx, cy, cz)
        camera.lookAt(0, 16, 0)
      }

      // Idle animation when not dragging
      if (parts) {
        if (!isDraggingRef.current) {
          // Slow ambient orbit drift
          targetYawRef.current += 0.0025 * autoRotateSpeed

          // Breathing / slight vertical bobbing
          const breath = Math.sin(elapsedTime * 2.2) * 0.18
          parts.modelRoot.position.y = breath

          // Subtle natural head sway
          parts.headGroup.rotation.y = Math.sin(elapsedTime * 1.0) * 0.05
          parts.headGroup.rotation.x = Math.sin(elapsedTime * 1.4) * 0.03

          // Gentle natural arm swing
          parts.leftArmGroup.rotation.x = Math.sin(elapsedTime * 1.6) * 0.06
          parts.rightArmGroup.rotation.x = -Math.sin(elapsedTime * 1.6) * 0.06
          parts.leftArmGroup.rotation.z = Math.abs(Math.sin(elapsedTime * 2.2)) * 0.02
          parts.rightArmGroup.rotation.z = -Math.abs(Math.sin(elapsedTime * 2.2)) * 0.02
        } else {
          // Smoothly return body joints to resting neutral pose while dragging
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

  // 5. Mouse and Touch Drag Controls for 360 Rotation
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

    // Rotasi hanya horizontal 360 derajat (kiri-kanan saja, tanpa atas-bawah)
    targetYawRef.current += dx * 0.012
    targetPitchRef.current = 0
  }

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>): void => {
    isDraggingRef.current = false
    setIsDragging(false)
    try {
      ;(e.target as HTMLElement).releasePointerCapture(e.pointerId)
    } catch {
      // Ignore if pointer capture already ended
    }
  }

  useEffect(() => {
    const el = containerRef.current
    if (!el || !interactive) return

    const onWheel = (e: WheelEvent): void => {
      e.preventDefault()
      // Clamp zoom distance between 36 and 76
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
      aria-label="Minecraft player skin preview"
      tabIndex={0}
      title={
        webglFailed
          ? undefined
          : interactive
            ? 'Drag to rotate 360 degrees, scroll to zoom'
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
          label="2D Minecraft player skin preview"
        />
      ) : null}
    </div>
  )
}
