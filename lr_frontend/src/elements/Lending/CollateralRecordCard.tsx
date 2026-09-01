import { AlertTriangle, ExternalLink, ShieldCheck } from 'lucide-react'
import { formatDate, pesos, xlm, xlmRate } from '../../functions/Lending/money'
import type { CollateralMovement, CollateralRecord } from '../../functions/Lending/types'

/**
 * The custody record for one XLM-collateral loan: what is locked, the price it
 * was struck at with the feeds behind it, and every on-chain movement linked
 * to Stellar Expert.
 *
 * The point of this panel is that none of it has to be believed. Every
 * movement carries the transaction that made it, and every price carries the
 * providers that agreed on it — so a borrower (or a reviewer) can check the
 * chain instead of trusting the screen. It is a window, never a calculator:
 * every number here is the engine's.
 */

const NETWORK = import.meta.env.VITE_STELLAR_NETWORK === 'public' ? 'public' : 'testnet'
const txLink = (hash: string) => `https://stellar.expert/explorer/${NETWORK}/tx/${hash}`
const contractLink = (id: string) => `https://stellar.expert/explorer/${NETWORK}/contract/${id}`

const MOVEMENT_LABEL: Record<CollateralMovement['kind'], string> = {
    lock: 'Locked into the vault',
    mark_repaid: 'Repayment recorded on-chain',
    release: 'Released to your wallet',
    mark_defaulted: 'Default recorded on-chain',
    seize: 'Seized to the treasury',
}

/** "GABC…WXYZ" — enough to recognise a wallet, short enough to sit in a row. */
const shortKey = (key: string) => `${key.slice(0, 5)}…${key.slice(-4)}`

export default function CollateralRecordCard({ record }: { record: CollateralRecord }) {
    const { price } = record
    const ratioPct = record.collateral_ratio_bps / 100

    return (
        <div className='lending-custody'>
            <div className='lending-custody-head'>
                <span className='lending-card-icon is-accent'><ShieldCheck /></span>
                <h3>Collateral record</h3>
                <span className={`lending-custody-status is-${record.status}`}>{record.status}</span>
            </div>

            <dl className='lending-custody-grid'>
                <div>
                    <dt>Locked</dt>
                    <dd>
                        {xlm(record.locked_stroops || record.required_stroops)}
                        {record.locked_stroops === 0 && <span className='lending-muted'> (required)</span>}
                    </dd>
                </div>
                <div>
                    <dt>Covering</dt>
                    <dd>{pesos(record.principal)} at {ratioPct}%</dd>
                </div>
                <div>
                    <dt>Worth now</dt>
                    <dd>
                        {record.value_centavos === null ? '—' : pesos(record.value_centavos)}
                        {record.health_pct !== null && (
                            <span className={record.liquidatable ? ' lending-liquidation' : ' lending-muted'}>
                                {' '}· health {record.health_pct}%
                            </span>
                        )}
                    </dd>
                </div>
                <div>
                    <dt>From wallet</dt>
                    <dd title={record.wallet_address}>{shortKey(record.wallet_address)}</dd>
                </div>
            </dl>

            {record.liquidatable && (
                <p className='lending-muted lending-liquidation'>
                    <AlertTriangle />
                    Below the liquidation threshold. Top-up isn’t supported — repay to protect it.
                </p>
            )}

            {/* The pinned rate. Never recomputed — this is the number the
                requirement was struck at, and the one the vault contract
                checked against a public feed before accepting the coins. */}
            {price.centavos_per_xlm !== null && (
                <div className='lending-custody-price'>
                    <p className='lending-muted'>
                        Priced at <b>{xlmRate(price.centavos_per_xlm)}</b>
                        {price.priced_at !== null && <> on {formatDate(price.priced_at)}</>}
                        {price.sources.length > 0 && (
                            <> — {price.sources_used} of {price.sources.length} feeds agreed</>
                        )}
                        {price.usd_per_xlm_e8 !== null && price.usd_php_centavos !== null && (
                            <>
                                {' '}(${(price.usd_per_xlm_e8 / 1e8).toFixed(6)} / XLM
                                × {pesos(price.usd_php_centavos)} / USD)
                            </>
                        )}
                    </p>

                    {price.sources.length > 0 && (
                        <div className='lending-schedule-scroll'>
                            <table className='lending-schedule lending-feeds'>
                                <thead>
                                    <tr><th>Feed</th><th>Pair</th><th>Rate</th><th>Off median</th><th>Counted</th></tr>
                                </thead>
                                <tbody>
                                    {price.sources.map(source => (
                                        <tr key={source.name} className={source.used ? undefined : 'is-dropped'}>
                                            <td>{source.name}</td>
                                            <td>{source.leg}</td>
                                            <td>{xlmRate(source.centavos_per_xlm)}</td>
                                            <td>{source.deviation_bps} bps</td>
                                            <td>{source.used ? 'yes' : 'dropped'}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    )}
                </div>
            )}

            {record.movements.length > 0 && (
                <ul className='lending-moves'>
                    {record.movements.map((move, i) => (
                        <li key={`${move.kind}-${i}`} className='lending-move'>
                            <div className='lending-move-what'>
                                <b>{MOVEMENT_LABEL[move.kind]}</b>
                                {move.at !== null && <span className='lending-muted'> · {formatDate(move.at)}</span>}
                                {move.quote_php_per_xlm_centavos !== null && (
                                    <span className='lending-muted'>
                                        {' '}· checked at {xlmRate(move.quote_php_per_xlm_centavos)}
                                    </span>
                                )}
                            </div>
                            {move.tx_hash ? (
                                <a
                                    className='lending-move-link'
                                    href={txLink(move.tx_hash)}
                                    target='_blank'
                                    rel='noreferrer noopener'
                                >
                                    {shortKey(move.tx_hash)} <ExternalLink aria-hidden='true' />
                                </a>
                            ) : (
                                <span className='lending-move-status'>{move.status}</span>
                            )}
                        </li>
                    ))}
                </ul>
            )}

            {record.contract_id && (
                <p className='lending-muted'>
                    Held by vault{' '}
                    <a href={contractLink(record.contract_id)} target='_blank' rel='noreferrer noopener'>
                        {shortKey(record.contract_id)}
                    </a>
                    . Only the platform key can release or seize it, and only after the outcome is recorded
                    on-chain — both movements show up in this list when they happen.
                </p>
            )}
        </div>
    )
}
