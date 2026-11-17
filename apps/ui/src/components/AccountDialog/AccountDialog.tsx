import { useRef, useState } from 'react'
import type { FormEvent, ReactNode } from 'react'
import { LauncherError, usernameSchema } from '@mc/shared'
import type { DeviceCodeInfo } from '@mc/shared'
import { useI18n } from '../../i18n'
import { errorMessage, invoke } from '../../api/launcher'
import {
  useAccounts,
  useBeginMicrosoftLogin,
  useCompleteMicrosoftLogin,
  useCreateOfflineAccount,
  useSetActiveAccount,
} from '../../api/queries'
import { useAppStore } from '../../stores/app-store'
import { GlassDialog } from '../GlassDialog/GlassDialog'
import { GlassButton } from '../GlassButton/GlassButton'
import styles from './AccountDialog.module.css'

export interface AccountDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** Form akun: offline (username) dan Microsoft (device code). */
export function AccountDialog({ open, onOpenChange }: AccountDialogProps): ReactNode {
  const { t } = useI18n()
  const accounts = useAccounts()
  const createAccount = useCreateOfflineAccount()
  const setActiveAccount = useSetActiveAccount()
  const beginMicrosoft = useBeginMicrosoftLogin()
  const completeMicrosoft = useCompleteMicrosoftLogin()
  const pushToast = useAppStore((state) => state.pushToast)
  const [username, setUsername] = useState('')
  const [validationError, setValidationError] = useState<string | null>(null)
  const [deviceCode, setDeviceCode] = useState<DeviceCodeInfo | null>(null)
  const [microsoftError, setMicrosoftError] = useState<string | null>(null)
  const microsoftTaskIdRef = useRef<string | null>(null)
  const microsoftAbortedRef = useRef(false)

  const busy = createAccount.isPending || setActiveAccount.isPending
  const microsoftBusy = beginMicrosoft.isPending || completeMicrosoft.isPending

  const cancelMicrosoftSignIn = (): void => {
    microsoftAbortedRef.current = true
    const taskId = microsoftTaskIdRef.current
    if (!taskId) return
    microsoftTaskIdRef.current = null
    void invoke('tasks:cancel', { taskId }).catch(() => undefined)
  }

  const reset = (): void => {
    cancelMicrosoftSignIn()
    setUsername('')
    setValidationError(null)
    setDeviceCode(null)
    setMicrosoftError(null)
  }

  const handleOpenChange = (next: boolean): void => {
    if (!next) {
      reset()
    }
    onOpenChange(next)
  }

  const submit = (event: FormEvent<HTMLFormElement>): void => {
    event.preventDefault()
    const parsed = usernameSchema.safeParse(username)
    if (!parsed.success) {
      setValidationError(t('account.usernameInvalid'))
      return
    }
    const taken = accounts.data?.some(
      (account) => account.username.toLowerCase() === parsed.data.toLowerCase(),
    )
    if (taken) {
      setValidationError(t('account.usernameTaken'))
      return
    }

    setValidationError(null)
    createAccount.mutate(parsed.data, {
      onSuccess: async (account) => {
        try {
          await setActiveAccount.mutateAsync(account.id)
        } catch (error) {
          setValidationError(errorMessage(error, t('error.unknown')))
          return
        }
        pushToast(t('settings.accountCreated'))
        handleOpenChange(false)
      },
      onError: (error) => {
        setValidationError(errorMessage(error, t('error.unknown')))
      },
    })
  }

  const describeMicrosoftError = (error: unknown): string =>
    error instanceof LauncherError && error.code === 'PROVIDER_NOT_CONFIGURED'
      ? t('account.microsoftNotConfigured')
      : errorMessage(error, t('error.unknown'))

  const openVerificationPage = (url: string): void => {
    void invoke('shell:openExternal', { url }).catch(() => undefined)
  }

  const startMicrosoft = (): void => {
    microsoftAbortedRef.current = false
    setMicrosoftError(null)
    beginMicrosoft.mutate(undefined, {
      onSuccess: (code) => {
        if (microsoftAbortedRef.current) return
        setDeviceCode(code)
        openVerificationPage(code.verificationUri)
        const taskId =
          typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
            ? crypto.randomUUID()
            : `ms-auth-${Date.now()}-${Math.random().toString(16).slice(2)}`
        microsoftTaskIdRef.current = taskId
        completeMicrosoft.mutate(
          { ...code, taskId },
          {
            onSuccess: async (account) => {
              microsoftTaskIdRef.current = null
              try {
                await setActiveAccount.mutateAsync(account.id)
              } catch {
                // Account is already saved and selectable from the account menu.
              }
              pushToast(t('account.microsoftSuccess', { username: account.username }))
              handleOpenChange(false)
            },
            onError: (error) => {
              microsoftTaskIdRef.current = null
              if (error instanceof LauncherError && error.code === 'CANCELLED') {
                setDeviceCode(null)
                return
              }
              setMicrosoftError(describeMicrosoftError(error))
            },
          },
        )
      },
      onError: (error) => {
        setMicrosoftError(describeMicrosoftError(error))
      },
    })
  }

  return (
    <GlassDialog
      open={open}
      onOpenChange={handleOpenChange}
      title={t('account.createTitle')}
      description={t('account.createHint')}
      closeLabel={t('common.close')}
      size="sm"
      footer={
        <>
          <GlassButton variant="ghost" onClick={() => handleOpenChange(false)} disabled={busy}>
            {t('common.cancel')}
          </GlassButton>
          <GlassButton
            variant="primary"
            type="submit"
            form="create-offline-account"
            disabled={busy}
          >
            {busy ? t('common.loading') : t('common.create')}
          </GlassButton>
        </>
      }
    >
      <form id="create-offline-account" className={styles.form} onSubmit={submit}>
        <label className={styles.label} htmlFor="offline-username">
          {t('account.username')}
        </label>
        <input
          id="offline-username"
          className={styles.input}
          value={username}
          onChange={(event) => {
            setUsername(event.target.value)
            setValidationError(null)
          }}
          maxLength={16}
          autoFocus
          autoComplete="off"
          spellCheck={false}
          disabled={busy}
          aria-invalid={validationError ? true : undefined}
          aria-describedby={
            validationError ? 'offline-username-error' : 'offline-username-hint'
          }
        />
        {validationError ? (
          <p className={styles.error} id="offline-username-error" role="alert">
            {validationError}
          </p>
        ) : (
          <p className={styles.hint} id="offline-username-hint">
            {t('account.usernamePlaceholder')}
          </p>
        )}
      </form>

      <div className={styles.divider} aria-hidden />
      <section className={styles.microsoft} aria-labelledby="microsoft-signin-title">
        <h3 className={styles.microsoftTitle} id="microsoft-signin-title">
          {t('account.microsoftSignIn')}
        </h3>
        <p className={styles.hint}>{t('account.microsoftHint')}</p>
        {deviceCode ? (
          <div className={styles.codeBox}>
            <code className={styles.codeValue} data-testid="microsoft-user-code">
              {t('account.microsoftCode', { code: deviceCode.userCode })}
            </code>
            <div style={{ display: 'flex', gap: '6px' }}>
              <GlassButton
                size="sm"
                variant="ghost"
                onClick={() => {
                  void navigator.clipboard.writeText(deviceCode.userCode)
                  pushToast(t('account.codeCopied'))
                }}
              >
                {t('account.copyCode')}
              </GlassButton>
              <GlassButton
                size="sm"
                variant="ghost"
                onClick={() => openVerificationPage(deviceCode.verificationUri)}
              >
                {t('account.microsoftOpen')}
              </GlassButton>
            </div>
          </div>
        ) : null}
        {microsoftError ? (
          <p className={styles.error} role="alert" data-testid="microsoft-error">
            {microsoftError}
          </p>
        ) : null}
        <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
          <GlassButton
            variant="outline"
            size="sm"
            onClick={startMicrosoft}
            disabled={microsoftBusy || busy}
            data-testid="microsoft-signin"
          >
            {completeMicrosoft.isPending ? t('account.microsoftWaiting') : t('account.microsoftSignIn')}
          </GlassButton>
          {completeMicrosoft.isPending ? (
            <GlassButton
              variant="ghost"
              size="sm"
              onClick={cancelMicrosoftSignIn}
              data-testid="microsoft-cancel"
            >
              {t('account.microsoftCancel')}
            </GlassButton>
          ) : null}
        </div>
      </section>
    </GlassDialog>
  )
}
