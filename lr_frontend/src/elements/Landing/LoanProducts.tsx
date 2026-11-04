import { useEffect, useRef } from 'react'
import { Coins, UserRoundCheck, Vault } from 'lucide-react'
import { useSurfaceTilt } from './useSurfaceTilt'

const PRODUCTS = [
    { icon: Vault, title: 'Deposit-backed', description: 'Secured by your own deposits' },
    { icon: Coins, title: 'XLM collateral-backed', description: '120% collateral coverage in XLM' },
    { icon: UserRoundCheck, title: 'Guarantor-backed', description: 'Backed by a member who vouches for you' },
]

function LoanProducts() {
    const container = useRef<HTMLDivElement>(null)
    useSurfaceTilt(container, -4)

    useEffect(() => {
        const element = container.current
        if (!element || !('IntersectionObserver' in window)) return

        const observer = new IntersectionObserver(([entry]) => {
            if (!entry.isIntersecting) return
            element.classList.add('is-visible')
            observer.disconnect()
        }, { threshold: 0.12 })
        observer.observe(element)
        return () => observer.disconnect()
    }, [])

    return (
        <div className='loan-products-wrap' id='loan-products' ref={container}>
            <section className='loan-products' aria-labelledby='loan-products-title'>
                <div className='loan-products__eyebrow'>
                    <span>Loan products</span>
                    <span className='loan-products__badge'>Stellar-native</span>
                </div>
                <h2 className='loan-products__title' id='loan-products-title'>Three ways to borrow</h2>
                <ul className='loan-products__list'>
                    {PRODUCTS.map(({ icon: Icon, title, description }) => (
                        <li className='loan-product' key={title}>
                            <span className='loan-product__icon'><Icon size={18} strokeWidth={1.5} aria-hidden='true' /></span>
                            <div className='loan-product__copy'>
                                <h3>{title}</h3>
                                <p>{description}</p>
                            </div>
                        </li>
                    ))}
                </ul>
                <div className='loan-products__credit'>
                    <div className='loan-products__credit-label'>
                        <span>Credit score</span>
                        <strong>50–150 scale</strong>
                    </div>
                    <div className='loan-products__scale' aria-hidden='true'><span /></div>
                    <div className='loan-products__ticks' aria-hidden='true'><span>50</span><span>100</span><span>150</span></div>
                </div>
            </section>
        </div>
    )
}

export default LoanProducts
