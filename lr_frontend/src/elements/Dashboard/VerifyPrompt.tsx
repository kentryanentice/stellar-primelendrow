import { useNavigate } from 'react-router-dom'
import { Hourglass, ShieldQuestion } from 'lucide-react'

/**
 * What a Pending or Verifying account sees instead of the overview.
 * AccessProvider lets both onto /dashboard (it's where sign-in lands), but
 * every figure on the overview belongs to member features they're still gated
 * out of — so the page says what unlocks it instead. Same split as the
 * sidebar's status card: Pending can act, Verifying can only wait.
 */
function VerifyPrompt({ verifying }: { verifying: boolean }) {
    const navigate = useNavigate()
    const Icon = verifying ? ShieldQuestion : Hourglass

    return (
        <section className='lending-card dash-locked'>
            <div className='lending-empty'>
                <div className='lending-empty-icon'><Icon aria-hidden='true' /></div>
                <p className='lending-empty-title'>
                    {verifying ? 'Your verification is under review' : 'Verify your identity to get started'}
                </p>
                <p className='lending-muted'>
                    {verifying
                        ? 'We’ll unlock lending, borrowing and your dashboard as soon as it’s approved.'
                        : 'Once you’re verified, your balance, loans and credit score all show up here.'}
                </p>
                {!verifying && (
                    <button type='button' className='lending-btn-primary' onClick={() => navigate('/verification')}>
                        Verify account
                    </button>
                )}
            </div>
        </section>
    )
}

export default VerifyPrompt
