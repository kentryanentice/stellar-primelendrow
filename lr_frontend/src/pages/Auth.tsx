import BrandPanel from '../elements/Auth/BrandPanel'
import ForgotScreen from '../elements/Auth/ForgotScreen'
import LoginScreen from '../elements/Auth/LoginScreen'
import RegisterScreen from '../elements/Auth/RegisterScreen'
import ResetScreen from '../elements/Auth/ResetScreen'
import TermsModal from '../elements/Auth/TermsModal'
import VerifyScreen from '../elements/Auth/VerifyScreen'
import useAuthFunctions from '../functions/Auth/AuthFunctions'

function Auth() {
    const auth = useAuthFunctions()
    const { screen, termsOpen, submit } = auth

    return (
        <main className='auth'>
            <img className='auth-bg' src='/pictures/hero-bg.png' alt='' />
            <section className='auth-card'>
                <div className='auth-panel'>
                    <form onSubmit={submit}>
                        {screen === 'login' && <LoginScreen {...auth} />}
                        {screen === 'register' && <RegisterScreen {...auth} />}
                        {screen === 'verify' && <VerifyScreen {...auth} />}
                        {screen === 'forgot' && <ForgotScreen {...auth} />}
                        {screen === 'reset' && <ResetScreen {...auth} />}
                    </form>
                </div>

                <BrandPanel busy={auth.busy} />

                {termsOpen && <TermsModal {...auth} />}
            </section>
        </main>
    )
}

export default Auth
