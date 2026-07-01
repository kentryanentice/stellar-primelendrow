import type { AuthState } from './types'
import { EyeIcon } from './icons'

type LoginScreenProps = Pick<AuthState, 'login' | 'setLogin' | 'showLoginPw' | 'toggleLoginPw' | 'goForgot' | 'goRegister' | 'busy'>

export default function LoginScreen({ login, setLogin, showLoginPw, toggleLoginPw, goForgot, goRegister, busy }: LoginScreenProps) {
    return (
        <div className='auth-view'>
            <h1 className='auth-title'>Welcome back</h1>
            <p className='auth-sub'>Sign in to access your lending dashboard.</p>

            <label className='auth-label'>Email</label>
            <div className='auth-row-gap'>
                <input className='auth-input' value={login.email} onChange={e => setLogin({ ...login, email: e.target.value })} type='email' placeholder='you@company.com' required />
            </div>

            <label className='auth-label'>Password</label>
            <div className='auth-pw'>
                <input className='auth-input' value={login.password} onChange={e => setLogin({ ...login, password: e.target.value })} type={showLoginPw ? 'text' : 'password'} placeholder='Enter your password' required />
                <button className='auth-eye' type='button' onClick={toggleLoginPw} aria-label='Toggle password'><EyeIcon off={showLoginPw} /></button>
            </div>

            <div className='auth-meta'>
                <button className='auth-link' type='button' onClick={goForgot}>Forgot password?</button>
            </div>

            <button className='auth-btn' type='submit' disabled={busy}>{busy ? 'Please wait…' : 'Log In'}</button>

            <p className='auth-switch'>Don't have an account? <button className='auth-link' type='button' onClick={goRegister}>Create one</button></p>
        </div>
    )
}
