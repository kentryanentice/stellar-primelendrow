import type { ReceiptLine } from '../../functions/Lending/receipt'

/**
 * One movement's receipt: the order, line by line, down to what landed.
 *
 * `reference` is the provider's own confirmation — a PayPal capture or
 * transfer id — shown in full here because the row above can only fit a
 * shortened one, and a member checking a payment against their PayPal account
 * needs the whole thing.
 */
function Receipt({ lines, reference }: { lines: ReceiptLine[]; reference?: string | null }) {
    return (
        <div className='lending-receipt'>
            <dl className='lending-receipt-lines'>
                {lines.map(line => (
                    <div
                        key={line.label}
                        className={`lending-receipt-line${line.total ? ' is-total' : ''}${line.muted ? ' is-muted' : ''}`}
                    >
                        <dt>{line.label}</dt>
                        <dd>{line.value}</dd>
                    </div>
                ))}
            </dl>
            {reference && (
                <p className='lending-receipt-ref'>
                    <span>Confirmation</span>
                    <code>{reference}</code>
                </p>
            )}
        </div>
    )
}

export default Receipt
