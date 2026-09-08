import { CreditCard, Loader2 } from 'lucide-react'
import useStripeCheckout, { type CheckoutPurpose } from '../../functions/Lending/useStripeCheckout'

type StripeButtonProps = {
    /** Whole centavos to charge; null disables the button (nothing valid typed yet). */
    amountCentavos: number | null
    purpose: CheckoutPurpose
    /** Required when `purpose` is 'repay'. */
    loanId?: string
    label?: string
}

/**
 * Starts a Stripe payment. The counterpart of PayPalButton, and deliberately
 * much smaller.
 *
 * PayPalButton has to load an SDK, render an iframe, hold the amount in a ref
 * so keystrokes don't tear the iframe down, and create the order in the
 * browser. None of that exists here: the engine creates the Checkout Session —
 * deciding the amount and stamping in who is paying — and this component's
 * whole job is to ask for it and navigate. There is nothing in this file a
 * tampered page could change that would alter what gets charged or credited.
 *
 * The cost is that the member leaves the app to pay. `ManageFundsCard` picks
 * them back up on return.
 */
function StripeButton({ amountCentavos, purpose, loanId, label }: StripeButtonProps) {
    const { startCheckout, starting } = useStripeCheckout()

    return (
        <button
            type='button'
            className='lending-btn'
            disabled={!amountCentavos || starting}
            aria-disabled={!amountCentavos}
            onClick={() => {
                if (!amountCentavos) return
                void startCheckout(purpose, amountCentavos, loanId)
            }}
        >
            {starting ? <Loader2 className='settings-wallet-spin' /> : <CreditCard />}
            {starting ? 'Opening Stripe…' : (label ?? 'Pay with card')}
        </button>
    )
}

export default StripeButton
