import { useState } from 'react'
import type { ReactNode } from 'react'
import { useNavigate } from 'react-router'
import { Check, LogOut, Plus, Settings2 } from 'lucide-react'
import { DropdownMenu } from 'radix-ui'
import { useI18n } from '../../i18n'
import { errorMessage } from '../../api/launcher'
import { useAccounts, useSetActiveAccount, useSettings, useSignOut } from '../../api/queries'
import { useAppStore } from '../../stores/app-store'
import { AccountDialog } from '../AccountDialog/AccountDialog'
import { PlayerHeadAvatar } from '../MinecraftSkin/PlayerHeadAvatar'
import styles from './AccountMenu.module.css'

export interface AccountMenuProps {
  trigger: ReactNode
  align?: 'start' | 'center' | 'end'
}

/** Menu ganti/buat akun. Item akun menulis activeAccountId lewat IPC nyata. */
export function AccountMenu({ trigger, align = 'end' }: AccountMenuProps): ReactNode {
  const { t } = useI18n()
  const navigate = useNavigate()
  const accounts = useAccounts()
  const settings = useSettings()
  const setActiveAccount = useSetActiveAccount()
  const signOut = useSignOut()
  const pushToast = useAppStore((state) => state.pushToast)
  const [createOpen, setCreateOpen] = useState(false)

  const activate = (id: string, username: string): void => {
    setActiveAccount.mutate(id, {
      onSuccess: () => {
        pushToast(t('toast.accountActive', { username }))
      },
      onError: (error) => {
        pushToast(errorMessage(error, t('error.unknown')), 'error')
      },
    })
  }

  const activeId = settings.data?.activeAccountId ?? null

  return (
    <>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>{trigger}</DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content className={styles.content} sideOffset={8} align={align}>
            <DropdownMenu.Label className={styles.label}>
              {t('account.manage')}
            </DropdownMenu.Label>
            {accounts.data && accounts.data.length > 0 ? (
              accounts.data.map((account) => (
                <DropdownMenu.Group key={account.id}>
                  <DropdownMenu.Item
                    className={styles.item}
                    onSelect={() => activate(account.id, account.username)}
                  >
                    <PlayerHeadAvatar
                      account={account}
                      isActiveAccount={account.id === activeId}
                      size={26}
                      className={styles.avatar}
                    />
                    <span className={styles.itemText}>
                      <span className={styles.username}>{account.username}</span>
                      <span className={styles.type}>
                        {account.type === 'offline'
                          ? t('account.type.offline')
                          : t('account.type.microsoft')}
                      </span>
                    </span>
                    {account.id === activeId ? (
                      <Check size={15} className={styles.check} aria-hidden />
                    ) : null}
                  </DropdownMenu.Item>
                  {account.type === 'microsoft' ? (
                    <DropdownMenu.Item
                      className={`${styles.item} ${styles.dangerItem}`}
                      onSelect={() => {
                        signOut.mutate(account.id, {
                          onSuccess: () => {
                            pushToast(t('account.signedOut', { username: account.username }))
                          },
                          onError: (error) => {
                            pushToast(errorMessage(error, t('error.unknown')), 'error')
                          },
                        })
                      }}
                    >
                      <LogOut size={15} aria-hidden />
                      <span className={styles.itemText}>{t('account.signOut')}</span>
                    </DropdownMenu.Item>
                  ) : null}
                </DropdownMenu.Group>
              ))
            ) : (
              <DropdownMenu.Item className={styles.item} disabled>
                <span className={styles.itemText}>
                  <span className={styles.type}>{t('account.none')}</span>
                </span>
              </DropdownMenu.Item>
            )}

            <DropdownMenu.Separator className={styles.separator} />

            <DropdownMenu.Item
              className={styles.item}
              onSelect={() => setCreateOpen(true)}
            >
              <Plus size={15} aria-hidden />
              <span className={styles.itemText}>{t('account.create')}</span>
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className={styles.item}
              onSelect={() => navigate('/settings')}
            >
              <Settings2 size={15} aria-hidden />
              <span className={styles.itemText}>{t('settings.accounts')}</span>
            </DropdownMenu.Item>
            <DropdownMenu.Item className={styles.note} disabled>
              {t('account.microsoftNote')}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      <AccountDialog open={createOpen} onOpenChange={setCreateOpen} />
    </>
  )
}
