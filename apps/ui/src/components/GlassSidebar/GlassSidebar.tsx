import { Suspense, lazy, useState } from 'react'
import type { ReactNode } from 'react'
import { NavLink } from 'react-router'
import {
  Compass,
  Images,
  LayoutGrid,
  ListTree,
  LogOut,
  Play,
  Plus,
  Settings,
  Shirt,
} from 'lucide-react'
import { useI18n } from '../../i18n'
import { useActiveAccount } from '../../api/queries'
import { useAppStore } from '../../stores/app-store'
import styles from './GlassSidebar.module.css'

const CreateInstanceDialog = lazy(() =>
  import('../CreateInstanceDialog/CreateInstanceDialog').then((m) => ({
    default: m.CreateInstanceDialog,
  })),
)

interface RailLinkProps {
  to: string
  label: string
  testId?: string
  end?: boolean
  children: (isActive: boolean) => ReactNode
}

function RailLink({ to, label, testId, end, children }: RailLinkProps): ReactNode {
  return (
    <NavLink
      to={to}
      end={end ?? false}
      className={({ isActive }) => `${styles.railItem} ${isActive ? styles.railItemActive : ''}`}
      aria-label={label}
      data-testid={testId}
    >
      {({ isActive }) => (
        <>
          {children(isActive)}
          <span className={styles.tooltip}>{label}</span>
        </>
      )}
    </NavLink>
  )
}

/**
 * Rail ikon kiri. Setiap ikon menuju layar yang benar-benar ada: beranda,
 * jelajahi, skin, tangkapan layar, library, dan konsol.
 */
export function GlassSidebar(): ReactNode {
  const { t } = useI18n()
  const { account } = useActiveAccount()
  const isGameRunning = useAppStore((state) => state.isGameRunning)

  const [createDialogOpen, setCreateDialogOpen] = useState(false)

  return (
    <nav className={styles.sidebar} aria-label={t('app.name')}>
      <div className={styles.group}>
        <RailLink to="/" end label={t('nav.home')} testId="nav-home">
          {(isActive) => (
            <Play
              size={19}
              strokeWidth={isActive ? 2.2 : 1.9}
              fill={isActive ? 'currentColor' : 'none'}
              aria-hidden
            />
          )}
        </RailLink>

        <RailLink to="/browse" label="Discover" testId="nav-browse">
          {(isActive) => <Compass size={19} strokeWidth={isActive ? 2.2 : 1.9} aria-hidden />}
        </RailLink>

        <RailLink to="/skins" label="Skins" testId="nav-skins">
          {(isActive) => <Shirt size={19} strokeWidth={isActive ? 2.2 : 1.9} aria-hidden />}
        </RailLink>

        <RailLink to="/screenshots" label="Screenshots" testId="nav-screenshots">
          {(isActive) => <Images size={19} strokeWidth={isActive ? 2.2 : 1.9} aria-hidden />}
        </RailLink>

        <RailLink to="/instances" label={t('nav.instances')} testId="nav-instances">
          {(isActive) => <LayoutGrid size={19} strokeWidth={isActive ? 2.2 : 1.9} aria-hidden />}
        </RailLink>

        <RailLink to="/console" label={t('nav.console')} testId="nav-console">
          {(isActive) => (
            <>
              <ListTree size={19} strokeWidth={isActive ? 2.2 : 1.9} aria-hidden />
              {isGameRunning ? <span className={styles.livePulse} /> : null}
            </>
          )}
        </RailLink>
      </div>

      <div className={styles.divider} role="presentation" />

      <div className={styles.group}>
        <RailLink to="/accounts" label={t('nav.manageAccounts')} testId="nav-accounts">
          {() => (
            <span className={`${styles.avatar} ${account ? styles.avatarFilled : ''}`} aria-hidden>
              {account ? account.username.slice(0, 2).toUpperCase() : '?'}
            </span>
          )}
        </RailLink>

        <button
          type="button"
          className={styles.railItem}
          onClick={() => setCreateDialogOpen(true)}
          aria-label="New instance"
          title="New instance"
        >
          <Plus size={19} strokeWidth={2.1} aria-hidden />
        </button>
      </div>

      <div className={styles.footer}>
        <RailLink to="/settings" label={t('nav.settings')} testId="nav-settings">
          {(isActive) => <Settings size={19} strokeWidth={isActive ? 2.2 : 1.9} aria-hidden />}
        </RailLink>

        <RailLink to="/accounts" label={t('nav.accounts')} testId="nav-signin">
          {() => <LogOut size={19} strokeWidth={1.9} aria-hidden />}
        </RailLink>
      </div>

      {createDialogOpen ? (
        <Suspense fallback={null}>
          <CreateInstanceDialog open={createDialogOpen} onOpenChange={setCreateDialogOpen} />
        </Suspense>
      ) : null}
    </nav>
  )
}
