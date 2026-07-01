import type { AuthState } from './types'

type TermsModalProps = Pick<AuthState, 'termsEnd' | 'closeTerms' | 'onTermsScroll' | 'acceptTerms'>

export default function TermsModal({ termsEnd, closeTerms, onTermsScroll, acceptTerms }: TermsModalProps) {
    return (
        <div className='auth-modal'>
            <div className='auth-modal-card'>
                <div className='auth-modal-head'>
                    <div>
                        <h3>Terms &amp; Conditions</h3>
                        <p>Scroll to the bottom to continue</p>
                    </div>
                    <button className='auth-modal-close' type='button' aria-label='Close' onClick={closeTerms}>✕</button>
                </div>
                <div className='auth-terms' onScroll={onTermsScroll}>
                    <p><strong>1. Acceptance.</strong> By creating a PrimeLendRow account you agree to use the platform lawfully and to keep your login credentials secure. You are responsible for activity that occurs under your account and for keeping your profile information accurate.</p>
                    <p><strong>2. Privacy.</strong> We process your personal data only to operate and improve the service. We never sell your information to third parties, and account data is handled according to our privacy practices. You may request account assistance through the support channels we provide.</p>
                    <p><strong>3. Communications.</strong> We may send transactional emails relating to your account, such as security alerts, verification codes, account updates, and password reset messages. These messages are required for account security and cannot be fully disabled while the account is active.</p>
                    <p><strong>4. Acceptable use.</strong> You agree not to misuse the service, interfere with access, attempt unsupported access methods, upload harmful content, or disrupt other users. You also agree not to reverse engineer, overload, or bypass platform protections.</p>
                    <p><strong>5. Liability.</strong> The service is provided as is. PrimeLendRow is not liable for indirect or consequential losses arising from your use of the platform, to the extent permitted by law. You should review all account activity before relying on it for decisions.</p>
                    <p><strong>6. Changes.</strong> We may update these terms from time to time. Continued use after an update constitutes acceptance of the revised terms. Please review any changes when prompted. You have now reached the end.</p>
                </div>
                <div className='auth-modal-foot'>
                    <span>{termsEnd ? 'Thanks for reading' : 'Keep scrolling…'}</span>
                    <button className='auth-modal-btn' type='button' disabled={!termsEnd} onClick={acceptTerms}>Agree &amp; Continue</button>
                </div>
            </div>
        </div>
    )
}
