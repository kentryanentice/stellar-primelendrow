import type { AuthState } from './types'
import { MailIcon } from './icons'

type VerifyScreenProps = Pick<AuthState,
    'reg' | 'otp' | 'otpRefs' | 'onOtpChange' | 'onOtpKey' | 'onOtpPaste' | 'resendIn' | 'resendOtp' | 'canVerify' | 'busy' | 'goRegister'
>

export default function VerifyScreen({ reg, otp, otpRefs, onOtpChange, onOtpKey, onOtpPaste, resendIn, resendOtp, canVerify, busy, goRegister }: VerifyScreenProps) {
    return (
        <div className='auth-view is-verify'>
            <div className='auth-mail'><MailIcon /></div>
            <h1 className='auth-title'>Check your email</h1>
            <p className='auth-sub'>We sent a 6-digit code to<br /><b>{reg.email || 'you@company.com'}</b></p>

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
                <button className='auth-link' type='button' onClick={resendIn ? undefined : resendOtp} disabled={!!resendIn || busy}>
                    {resendIn ? 'Change email' : 'Resend code'}
                </button>
            </div>

            <button className='auth-btn' type='submit' disabled={busy || !canVerify}>{busy ? 'Please wait…' : 'Verify & Continue'}</button>

            <p className='auth-switch'>Wrong address? <button className='auth-link' type='button' onClick={goRegister}>Change email</button></p>
        </div>
    )
}
