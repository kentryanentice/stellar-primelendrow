import type { AuthState } from './types'
import { CheckIcon, CrossIcon, EyeIcon } from './icons'

type RegisterScreenProps = Pick<AuthState,
    | 'reg' | 'setReg' | 'showRegPw' | 'toggleRegPw' | 'showConfirmPw' | 'toggleConfirmPw'
    | 'score' | 'strengthLabel' | 'match' | 'terms' | 'toggleTerms' | 'openTerms' | 'canRegister' | 'busy' | 'goLogin'
>

export default function RegisterScreen({
    reg, setReg, showRegPw, toggleRegPw, showConfirmPw, toggleConfirmPw,
    score, strengthLabel, match, terms, toggleTerms, openTerms, canRegister, busy, goLogin,
}: RegisterScreenProps) {
    return (
        <div className='auth-view'>
            <h1 className='auth-title'>Create your account</h1>
            <p className='auth-sub'>Get started in under two minutes.</p>

            <label className='auth-label'>Username</label>
            <div className='auth-row-gap-sm'>
                <input className='auth-input' value={reg.name} onChange={e => setReg({ ...reg, name: e.target.value })} placeholder='Choose a username' required />
            </div>

            <label className='auth-label'>Email</label>
            <div className='auth-row-gap-sm'>
                <input className='auth-input' value={reg.email} onChange={e => setReg({ ...reg, email: e.target.value })} type='email' placeholder='you@company.com' required />
            </div>

            <label className='auth-label'>Password</label>
            <div className='auth-pw'>
                <input className='auth-input' value={reg.password} onChange={e => setReg({ ...reg, password: e.target.value })} type={showRegPw ? 'text' : 'password'} placeholder='Create a password' required />
                <button className='auth-eye' type='button' onClick={toggleRegPw} disabled={busy} aria-label='Toggle password'><EyeIcon off={showRegPw} /></button>
            </div>
            <div className='auth-strength' data-level={score}>
                <div className='auth-strength-track'><div className='auth-strength-fill' /></div>
                <span className='auth-strength-label'>{strengthLabel}</span>
            </div>

            <label className='auth-label'>Confirm password</label>
            <div className='auth-pw'>
                <input className='auth-input' value={reg.confirm} onChange={e => setReg({ ...reg, confirm: e.target.value })} type={showConfirmPw ? 'text' : 'password'} placeholder='Re-enter your password' required />
                <button className='auth-eye' type='button' onClick={toggleConfirmPw} disabled={busy} aria-label='Toggle password'><EyeIcon off={showConfirmPw} /></button>
            </div>
            {match && (
                <div className={`auth-match ${match === 'ok' ? 'is-ok' : 'is-bad'}`}>
                    {match === 'ok' ? <CheckIcon /> : <CrossIcon />}
                    {match === 'ok' ? 'Passwords match' : 'Passwords do not match'}
                </div>
            )}

            <label className='auth-terms-check'>
                <input type='checkbox' checked={terms} onChange={toggleTerms} disabled={busy} />
                <span>I agree to the <button className='auth-link' type='button' onClick={openTerms} disabled={busy}>Terms &amp; Conditions</button></span>
            </label>

            <button className='auth-btn' type='submit' disabled={busy || !canRegister}>{busy ? 'Please wait…' : 'Create Account'}</button>

            <p className='auth-switch'>Already have an account? <button className='auth-link' type='button' onClick={goLogin} disabled={busy}>Log In</button></p>
        </div>
    )
}
