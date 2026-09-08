import { useEffect, useRef } from 'react'
import usePayPal from '../../functions/Lending/usePayPal'
import usePayPalOrder, { type OrderPurpose } from '../../functions/Lending/usePayPalOrder'
import { useToast } from '../../providers/useToast'

type PayPalButtonProps = {
    /** Whole centavos to charge; null disables the button (nothing valid typed yet). */
    amountCentavos: number | null
    /** What the payment is for — describes the order PayPal shows the member. */
    purpose?: OrderPurpose
    /** repay only: which loan the order describes. */
    loanId?: string
    /** Fires with the approved PayPal order id — the engine captures and verifies it server-side. */
    onApproved: (orderId: string) => void | Promise<void>
}

/**
 * Renders PayPal's hosted buttons into a container. The buttons are created
 * once per mount; the amount and the approval handler are read through refs
 * at click time, so typing a new amount doesn't tear down and re-render the
 * PayPal iframe on every keystroke.
 *
 * Security shape: this component only ever produces an ORDER ID. What was
 * actually paid is decided by the engine's server-side capture — a tampered
 * amount here changes the PayPal sheet, not what gets credited.
 */
function PayPalButton({ amountCentavos, purpose = 'deposit', loanId, onApproved }: PayPalButtonProps) {
    const { paypal, failed, configured } = usePayPal()
    const { createOrder } = usePayPalOrder()
    const toast = useToast()
    const containerRef = useRef<HTMLDivElement>(null)

    const amountRef = useRef(amountCentavos)
    const onApprovedRef = useRef(onApproved)
    const createOrderRef = useRef((centavos: number) => createOrder(centavos, purpose, loanId))
    // Refs are synced in an effect, not during render — required by the
    // React Compiler this repo builds with (a render-time ref write can be
    // memoized away silently).
    useEffect(() => {
        amountRef.current = amountCentavos
        onApprovedRef.current = onApproved
        createOrderRef.current = (centavos: number) => createOrder(centavos, purpose, loanId)
    })

    useEffect(() => {
        if (!paypal || !containerRef.current) return
        const buttons = paypal.Buttons({
            style: { layout: 'horizontal', height: 40, label: 'pay' },
            // The engine creates the order, not this page. That is what puts
            // the caller's ownership stamp on it (`custom_id`), which the
            // capture then checks — without it an order id is a bearer
            // reference and whoever presents it gets the money. It also means
            // the amount charged is the engine's, not this component's.
            createOrder: () => {
                const centavos = amountRef.current
                if (!centavos || centavos <= 0) {
                    toast.error('Enter a valid amount first')
                    return Promise.reject(new Error('no amount'))
                }
                return createOrderRef.current(centavos)
            },
            onApprove: async data => {
                await onApprovedRef.current(data.orderID)
            },
            onError: () => {
                toast.error('PayPal ran into a problem — nothing was charged, try again')
            },
        })
        void buttons.render(containerRef.current)
        return () => { void buttons.close() }
    }, [paypal, toast])

    if (!configured) {
        return <p className='lending-muted'>Payments aren’t configured on this deployment.</p>
    }
    if (failed) {
        return <p className='lending-muted'>Couldn’t load PayPal — check your connection and reload.</p>
    }
    return (
        <div
            ref={containerRef}
            className={`lending-paypal${amountCentavos ? '' : ' is-disabled'}`}
            aria-disabled={!amountCentavos}
        />
    )
}

export default PayPalButton
