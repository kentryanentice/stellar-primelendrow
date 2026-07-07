import { CheckCircle } from 'lucide-react'
import type { KYCState } from './types'
import { truncateAddress } from '../../functions/KYC/wallet'

type WalletStepProps = Pick<KYCState, 'walletAddress' | 'walletConnecting' | 'connectWallet' | 'disconnectWallet'>

export default function WalletStep({ walletAddress, walletConnecting, connectWallet, disconnectWallet }: WalletStepProps) {
    return (
        <div className='kyc-wallet-list'>
            <div className='kyc-wallet-row'>
                <div className='kyc-wallet-icon'>F</div>
                <p className='kyc-wallet-name'>Freighter</p>
                {walletAddress ? (
                    <span className='kyc-wallet-connected'><CheckCircle />Connected</span>
                ) : walletConnecting ? (
                    <span className='kyc-wallet-spinner' />
                ) : (
                    <button type='button' className='kyc-btn-outline' onClick={connectWallet}>Connect</button>
                )}
            </div>
            {walletAddress && (
                <p className='kyc-wallet-address'>
                    Address: <span title={walletAddress}>{truncateAddress(walletAddress)}</span> · <button type='button' onClick={disconnectWallet}>disconnect</button>
                </p>
            )}
        </div>
    )
}
