import type { AuthState } from './types'

type ForgotScreenProps = Pick<AuthState, 'reset' | 'setReset' | 'validResetEmail' | 'busy' | 'goLogin'>

export default function ForgotScreen({ reset, setReset, validResetEmail, busy, goLogin }: ForgotScreenProps) {
    return (
        <div className='auth-view'>
            <h1 className='auth-title'>Forgot your password?</h1>
            <p className='auth-sub'>Enter your email and we'll send you a 6-digit reset code.</p>

            <label className='auth-label'>Email</label>
            <div className='auth-row-gap'>
                <input className='auth-input' value={reset.email} onChange={e => setReset({ ...reset, email: e.target.value })} type='email' placeholder='you@company.com' required />
            </div>

            <button className='auth-btn' type='submit' disabled={busy || !validResetEmail}>{busy ? 'Please wait…' : 'Send Reset Code'}</button>

            <p className='auth-switch'>Remembered it? <button className='auth-link' type='button' onClick={goLogin}>Log In</button></p>
        </div>
    )
}
