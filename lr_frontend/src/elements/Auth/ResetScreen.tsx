import type { AuthState } from './types'
import { CheckIcon, CrossIcon, EyeIcon } from './icons'

type ResetScreenProps = Pick<AuthState,
    | 'reset' | 'setReset' | 'otp' | 'otpRefs' | 'onOtpChange' | 'onOtpKey' | 'onOtpPaste' | 'resendIn' | 'resendReset'
    | 'showResetPw' | 'toggleResetPw' | 'showResetConfirm' | 'toggleResetConfirm'
    | 'resetScore' | 'resetStrengthLabel' | 'resetMatch' | 'canReset' | 'busy' | 'goForgot'
>

export default function ResetScreen({
    reset, setReset, otp, otpRefs, onOtpChange, onOtpKey, onOtpPaste, resendIn, resendReset,
    showResetPw, toggleResetPw, showResetConfirm, toggleResetConfirm,
    resetScore, resetStrengthLabel, resetMatch, canReset, busy, goForgot,
}: ResetScreenProps) {
    return (
        <div className='auth-view is-reset'>
            <h1 className='auth-title'>Reset password</h1>
            <p className='auth-sub'>Enter the code sent to<br /><b>{reset.email || 'your email'}</b> and choose a new password.</p>

            <div className='auth-otp'>
                {otp.map((value, i) => (
                    <input
                        key={i}
                        className='auth-otp-box'
                        ref={el => { otpRefs.current[i] = el }}
                        value={value}
                        onChange={e => onOtpChange(i, e.target.value)}
                        onKeyDown={e => onOtpKey(i, e)}
                        onPaste={onOtpPaste}
                        inputMode='numeric'
                        maxLength={1}
                        aria-label={`Digit ${i + 1}`}
                    />
                ))}
            </div>

            <div className='auth-resend'>
                <span>{resendIn ? `Resend code in ${resendIn}s` : 'Didn’t get it?'}</span>
                <button className='auth-link' type='button' onClick={resendReset} disabled={!!resendIn || busy}>Resend code</button>
            </div>

            <label className='auth-label'>New password</label>
            <div className='auth-pw'>
                <input className='auth-input' value={reset.password} onChange={e => setReset({ ...reset, password: e.target.value })} type={showResetPw ? 'text' : 'password'} placeholder='Create a new password' required />
                <button className='auth-eye' type='button' onClick={toggleResetPw} disabled={busy} aria-label='Toggle password'><EyeIcon off={showResetPw} /></button>
            </div>
            <div className='auth-strength' data-level={resetScore}>
                <div className='auth-strength-track'><div className='auth-strength-fill' /></div>
                <span className='auth-strength-label'>{resetStrengthLabel}</span>
            </div>

            <label className='auth-label'>Confirm new password</label>
            <div className='auth-pw'>
                <input className='auth-input' value={reset.confirm} onChange={e => setReset({ ...reset, confirm: e.target.value })} type={showResetConfirm ? 'text' : 'password'} placeholder='Re-enter your new password' required />
                <button className='auth-eye' type='button' onClick={toggleResetConfirm} disabled={busy} aria-label='Toggle password'><EyeIcon off={showResetConfirm} /></button>
            </div>
            {resetMatch && (
                <div className={`auth-match ${resetMatch === 'ok' ? 'is-ok' : 'is-bad'}`}>
                    {resetMatch === 'ok' ? <CheckIcon /> : <CrossIcon />}
                    {resetMatch === 'ok' ? 'Passwords match' : 'Passwords do not match'}
                </div>
            )}

            <button className='auth-btn' type='submit' disabled={busy || !canReset}>{busy ? 'Please wait…' : 'Reset Password'}</button>

            <p className='auth-switch'>Wrong email? <button className='auth-link' type='button' onClick={goForgot} disabled={busy}>Start over</button></p>
        </div>
    )
}
