import { useCallback, useEffect, useRef, useState, type ClipboardEvent, type FormEvent, type KeyboardEvent, type UIEvent } from 'react'
import { useToast } from '../../providers/useToast'
import { useSession } from '../../providers/useSession'
import { useAccent } from '../../providers/AccentProvider'
import { loginRequest, registerRequest, verifyRequest, resetRequest, resetConfirmRequest } from './requests'

export type Screen = 'login' | 'register' | 'verify' | 'forgot' | 'reset'
export type Match = '' | 'ok' | 'bad'

const RESEND_WAIT = 60
const emptyOtp = () => ['', '', '', '', '', '']
const errorMessage = (err: unknown, fallback: string) => (err instanceof Error ? err.message : fallback)

/** Why a Google sign-in came back to this page. The engine only ever sends one
 *  of these codes (users::google), never free text, so nothing is echoed. */
const GOOGLE_ERRORS: Record<string, string> = {
    cancelled: 'Google sign-in was cancelled.',
    expired: 'That Google sign-in took too long or was opened in another browser. Please try again.',
    no_account: 'No PrimeLendRow account uses that Google email yet. Create one below.',
    link_password: 'An account already uses this email. Log in with your password; Google can only be linked automatically for Gmail and Google Workspace addresses.',
    email_unverified: "Your Google account's email isn't verified, so it can't be used here.",
    unavailable: "Google sign-in isn't available right now.",
    failed: 'Google sign-in failed. Please try again.',
}
const googleErrorInUrl = () => new URLSearchParams(window.location.search).get('oauth_error')

const STRENGTH_LABELS = ['Weak', 'Fair', 'Good', 'Strong']
/** [min length, has uppercase, has special, has digit] — backend requires the first three. */
const pwChecksOf = (pw: string) => [pw.length >= 8, /[A-Z]/.test(pw), /[^A-Za-z0-9]/.test(pw), /\d/.test(pw)]

/**
 * Drives the whole auth screen: state, derived flags, and the signed
 * login/register/verify actions. The page component only renders what this
 * returns — each async action owns its own `busy` state and try/catch.
 */
export default function useAuthFunctions() {
    const toast = useToast()
    const { setUser, setCsrfToken } = useSession()
    const { accent } = useAccent()

    // "No account yet" from Google lands straight on sign-up.
    const [screen, setScreen] = useState<Screen>(() => (googleErrorInUrl() === 'no_account' ? 'register' : 'login'))
    const [showLoginPw, setShowLoginPw] = useState(false)
    const [showRegPw, setShowRegPw] = useState(false)
    const [showConfirmPw, setShowConfirmPw] = useState(false)
    const [showResetPw, setShowResetPw] = useState(false)
    const [showResetConfirm, setShowResetConfirm] = useState(false)
    const [terms, setTerms] = useState(false)
    const [termsOpen, setTermsOpen] = useState(false)
    const [termsEnd, setTermsEnd] = useState(false)
    const [resendIn, setResendIn] = useState(0)
    const [busy, setBusy] = useState(false)
    const [login, setLogin] = useState({ email: '', password: '' })
    const [reg, setReg] = useState({ name: '', email: '', password: '', confirm: '' })
    const [reset, setReset] = useState({ email: '', password: '', confirm: '' })
    const [otp, setOtp] = useState<string[]>(emptyOtp)
    const otpRefs = useRef<(HTMLInputElement | null)[]>([])

    // ---- derived state ----
    const code = otp.join('')
    const pwChecks = pwChecksOf(reg.password)
    const score = pwChecks.filter(Boolean).length
    const strengthLabel = STRENGTH_LABELS[score - 1] ?? ''
    const match: Match = !reg.confirm ? '' : reg.password === reg.confirm ? 'ok' : 'bad'
    const validEmail = /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(reg.email)
    const canRegister = terms && !!reg.name.trim() && validEmail && pwChecks[0] && pwChecks[1] && pwChecks[2] && reg.password === reg.confirm
    const canVerify = code.length === 6

    // ---- reset (forgot password) derived state ----
    const resetChecks = pwChecksOf(reset.password)
    const resetScore = resetChecks.filter(Boolean).length
    const resetStrengthLabel = STRENGTH_LABELS[resetScore - 1] ?? ''
    const resetMatch: Match = !reset.confirm ? '' : reset.password === reset.confirm ? 'ok' : 'bad'
    const validResetEmail = /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(reset.email)
    const canReset = code.length === 6 && resetChecks[0] && resetChecks[1] && resetChecks[2] && reset.password === reset.confirm

    // ---- google sign-in result ----
    // Shown once, then taken out of the address bar so a refresh or a shared
    // link doesn't repeat it.
    useEffect(() => {
        const code = googleErrorInUrl()
        if (!code) return
        toast.error(GOOGLE_ERRORS[code] ?? GOOGLE_ERRORS.failed)
        const url = new URL(window.location.href)
        url.searchParams.delete('oauth_error')
        window.history.replaceState(null, '', `${url.pathname}${url.search}${url.hash}`)
    }, [toast])

    // ---- resend countdown ----
    useEffect(() => {
        if (!resendIn) return
        const timer = window.setTimeout(() => setResendIn(resendIn - 1), 1000)
        return () => window.clearTimeout(timer)
    }, [resendIn])

    // ---- navigation / toggles ----
    const goLogin = () => setScreen('login')
    const goRegister = () => { setScreen('register'); setOtp(emptyOtp()) }
    const goForgot = () => { setScreen('forgot'); setReset({ email: login.email, password: '', confirm: '' }); setOtp(emptyOtp()) }
    const toggleLoginPw = () => setShowLoginPw(v => !v)
    const toggleRegPw = () => setShowRegPw(v => !v)
    const toggleConfirmPw = () => setShowConfirmPw(v => !v)
    const toggleResetPw = () => setShowResetPw(v => !v)
    const toggleResetConfirm = () => setShowResetConfirm(v => !v)

    // ---- terms modal ----
    const openTerms = () => { setTermsEnd(false); setTermsOpen(true) }
    const closeTerms = () => setTermsOpen(false)
    const toggleTerms = () => (terms ? setTerms(false) : openTerms())
    const acceptTerms = () => { setTerms(true); setTermsOpen(false) }
    /** Marks the terms read once their end is on screen, and keeps them read:
     *  scrolling back up to recheck a clause no longer locks the button again.
     *  TermsModal also calls this on open and on resize, without any scrolling
     *  — terms short enough to fit the box never fire a scroll event, which
     *  left the button disabled for good on a tall screen. */
    const checkTermsEnd = useCallback((el: HTMLElement) => {
        if (el.scrollTop + el.clientHeight >= el.scrollHeight - 12) setTermsEnd(true)
    }, [])
    const onTermsScroll = (e: UIEvent<HTMLDivElement>) => checkTermsEnd(e.currentTarget)

    // ---- otp inputs ----
    const onOtpChange = (i: number, value: string) => {
        const digit = value.replace(/\D/g, '').slice(-1)
        setOtp(prev => { const next = prev.slice(); next[i] = digit; return next })
        if (digit && i < 5) otpRefs.current[i + 1]?.focus()
    }
    const onOtpKey = (i: number, e: KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Backspace' && !otp[i] && i > 0) otpRefs.current[i - 1]?.focus()
    }
    const onOtpPaste = (e: ClipboardEvent<HTMLInputElement>) => {
        const text = e.clipboardData.getData('text').replace(/\D/g, '').slice(0, 6)
        if (!text) return
        e.preventDefault()
        const next = emptyOtp()
        text.split('').forEach((c, j) => { next[j] = c })
        setOtp(next)
        otpRefs.current[Math.min(text.length, 5)]?.focus()
    }

    // ---- async actions (each owns its busy + try/catch) ----
    const handleLogin = async () => {
        setBusy(true)
        try {
            const { user, csrfToken } = await loginRequest(login.email, login.password)
            setUser(user)
            setCsrfToken(csrfToken)
            toast.success('Welcome back')
        } catch (err) {
            toast.error(errorMessage(err, 'Unable to log in'))
        } finally {
            setBusy(false)
        }
    }

    const handleRegister = async () => {
        setBusy(true)
        try {
            await registerRequest(reg.name, reg.email, reg.password, accent)
            setScreen('verify')
            setResendIn(RESEND_WAIT)
            setOtp(emptyOtp())
            toast.success('OTP sent to your email')
        } catch (err) {
            toast.error(errorMessage(err, 'Unable to create account'))
        } finally {
            setBusy(false)
        }
    }

    const handleVerify = async () => {
        setBusy(true)
        try {
            const { user, csrfToken } = await verifyRequest(reg.email, code)
            setUser(user)
            setCsrfToken(csrfToken)
            toast.success('Account created')
        } catch (err) {
            toast.error(errorMessage(err, 'Verification failed'))
        } finally {
            setBusy(false)
        }
    }

    const resendOtp = async () => {
        if (resendIn || busy) return
        setBusy(true)
        try {
            await registerRequest(reg.name, reg.email, reg.password, accent)
            setOtp(emptyOtp())
            setResendIn(RESEND_WAIT)
            toast.success('OTP resent')
        } catch (err) {
            toast.error(errorMessage(err, 'Unable to resend OTP'))
        } finally {
            setBusy(false)
        }
    }

    const handleForgot = async () => {
        setBusy(true)
        try {
            await resetRequest(reset.email, accent)
            setScreen('reset')
            setResendIn(RESEND_WAIT)
            setOtp(emptyOtp())
            toast.success('If that email is registered, a code has been sent')
        } catch (err) {
            toast.error(errorMessage(err, 'Unable to send reset code'))
        } finally {
            setBusy(false)
        }
    }

    const handleReset = async () => {
        setBusy(true)
        try {
            await resetConfirmRequest(reset.email, code, reset.password)
            toast.success('Password updated — please log in')
            setScreen('login')
            setReset({ email: '', password: '', confirm: '' })
            setOtp(emptyOtp())
        } catch (err) {
            toast.error(errorMessage(err, 'Unable to reset password'))
        } finally {
            setBusy(false)
        }
    }

    const resendReset = async () => {
        if (resendIn || busy) return
        setBusy(true)
        try {
            await resetRequest(reset.email, accent)
            setOtp(emptyOtp())
            setResendIn(RESEND_WAIT)
            toast.success('Code resent')
        } catch (err) {
            toast.error(errorMessage(err, 'Unable to resend code'))
        } finally {
            setBusy(false)
        }
    }

    const submit = (e: FormEvent) => {
        e.preventDefault()
        if (busy) return
        if (screen === 'login') return handleLogin()
        if (screen === 'register') return handleRegister()
        if (screen === 'verify') return handleVerify()
        if (screen === 'forgot') return handleForgot()
        return handleReset()
    }

    return {
        // view state
        screen, goLogin, goRegister, goForgot,
        login, setLogin, reg, setReg, reset, setReset,
        showLoginPw, showRegPw, showConfirmPw, toggleLoginPw, toggleRegPw, toggleConfirmPw,
        showResetPw, showResetConfirm, toggleResetPw, toggleResetConfirm,
        terms, termsOpen, termsEnd, openTerms, closeTerms, toggleTerms, acceptTerms, onTermsScroll, checkTermsEnd,
        otp, otpRefs, onOtpChange, onOtpKey, onOtpPaste,
        resendIn, busy,
        // derived
        score, strengthLabel, match, canRegister, canVerify,
        resetScore, resetStrengthLabel, resetMatch, validResetEmail, canReset,
        // actions
        submit, resendOtp, resendReset,
    }
}
