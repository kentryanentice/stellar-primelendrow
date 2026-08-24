import { Users } from 'lucide-react'
import useGuarantorInvites from '../../functions/Lending/useGuarantorInvites'
import { formatDate, pesos, rate } from '../../functions/Lending/money'

const STATUS_LABEL: Record<string, string> = {
    invited: 'Awaiting your answer',
    accepted: 'Pledge locked',
    declined: 'Declined',
    released: 'Released back to you',
    seized: 'Seized to cover a default',
}

/**
 * Invitations to vouch for someone's loan. The consent copy is explicit —
 * accepting freezes real deposit and it can be seized — and the accept POST
 * is what records that consent server-side (D1).
 */
function GuarantorCard({ onChanged }: { onChanged: () => void }) {
    const { invites, loading, error, respond, respondingId } = useGuarantorInvites(onChanged)

    // Quiet footer treatment when there's truly nothing to review — the
    // moment there's a real invite (or the fetch is loading/failed), this
    // opens back up into the full card below with its header, count pill,
    // and the actionable list.
    if (!loading && !error && invites.length === 0) {
        return (
            <div className='lending-guarantor-empty'>
                <span className='lending-guarantor-empty-icon'><Users aria-hidden='true' /></span>
                <p className='lending-muted'>
                    No guarantee requests yet. When someone asks you to guarantee a loan, you’ll decide here —
                    accepting locks part of your deposit for their loan.
                </p>
            </div>
        )
    }

    return (
        <section className='lending-card lending-card-guarantor'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><Users /></span>
                <h2>Guarantee requests</h2>
                {invites.some(i => i.status === 'invited') && (
                    <span className='lending-pill is-warn'>{invites.filter(i => i.status === 'invited').length} waiting</span>
                )}
            </div>

            {loading ? (
                <p className='lending-muted'>Loading invitations…</p>
            ) : error ? (
                <p className='lending-muted'>Couldn’t load invitations. Please try again later.</p>
            ) : (
                <ul className='lending-invites'>
                    {invites.map(invite => (
                        <li key={invite.id} className='lending-invite'>
                            <div className='lending-invite-body'>
                                <p className='lending-invite-title'>
                                    <b>{invite.borrower}</b> asks you to pledge <b>{pesos(invite.pledge_amount)}</b>
                                </p>
                                <p className='lending-muted'>
                                    {pesos(invite.amount)} guarantor loan · {rate(invite.rate_bps)} · {invite.term_months} mo · {formatDate(invite.created_at)}
                                </p>
                                {invite.status === 'invited' ? (
                                    <p className='lending-invite-warning'>
                                        Accepting freezes {pesos(invite.pledge_amount)} of your withdrawable deposit until the
                                        loan is repaid — and it can be <b>seized</b> if {invite.borrower} defaults.
                                    </p>
                                ) : (
                                    <p className='lending-muted'>{STATUS_LABEL[invite.status] ?? invite.status}</p>
                                )}
                            </div>
                            {invite.status === 'invited' && (
                                <div className='lending-invite-actions'>
                                    <button
                                        type='button'
                                        className='lending-btn-primary'
                                        disabled={respondingId === invite.id}
                                        onClick={() => respond(invite.id, true)}
                                    >
                                        {respondingId === invite.id ? 'Locking…' : 'Accept & pledge'}
                                    </button>
                                    <button
                                        type='button'
                                        className='lending-btn'
                                        disabled={respondingId === invite.id}
                                        onClick={() => respond(invite.id, false)}
                                    >
                                        Decline
                                    </button>
                                </div>
                            )}
                        </li>
                    ))}
                </ul>
            )}
        </section>
    )
}

export default GuarantorCard
