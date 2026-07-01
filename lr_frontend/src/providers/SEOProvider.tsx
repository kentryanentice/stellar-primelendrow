import { Helmet } from 'react-helmet-async'

type SEOProviderTypes = {
    title?: string
    description?: string
    keywords?: string
    image?: string
    noindex?: boolean
}

function SEOProvider({
    title = 'PrimeLendRow - Blockchain-Based Lend and Borrow Platform',
    description = 'PrimeLendRow is a blockchain-based lend and borrow platform that enables users to lend, borrow, and earn yield on digital assets through secure, transparent, and decentralized smart contracts.',
    keywords = 'PrimeLendRow, blockchain lending, crypto lending, decentralized lending, decentralized borrowing, DeFi lending, DeFi borrowing, lend and borrow platform, crypto loans, smart contract lending, web3 lending, digital asset lending, dApp, decentralized finance',
    image = '/pictures/primelendrow.webp',
    noindex = false,
}: SEOProviderTypes) {
    const canonical =
    typeof window !== 'undefined'
        ? `${window.location.origin}${window.location.pathname}`
        : ''

    return (
        <Helmet>
            <title>{title}</title>

            <meta name='description' content={description} />
            <meta name='keywords' content={keywords} />
            <meta name='robots' content={noindex ? 'noindex, nofollow' : 'index, follow'} />

            <link rel='canonical' href={canonical} />

            <meta property='og:title' content={title} />
            <meta property='og:description' content={description} />
            <meta property='og:type' content='website' />
            <meta property='og:url' content={canonical} />
            <meta property='og:image' content={image} />

            <meta name='twitter:card' content='summary_large_image' />
            <meta name='twitter:title' content={title} />
            <meta name='twitter:description' content={description} />
            <meta name='twitter:image' content={image} />
        </Helmet>
    )
}

export default SEOProvider
