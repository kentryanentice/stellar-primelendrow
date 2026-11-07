import type { RefObject } from 'react'
import { Link } from 'react-router-dom'
import type { LandingValues } from './useLandingPresentation'

type Props = { v: LandingValues; root: RefObject<HTMLDivElement | null> }

// The supplied design has distinct desktop and mobile compositions.
export default function LandingMobile({ v, root }: Props) {
    return (
<div style={{"minHeight": "100vh", "overflow": "clip", "fontFamily": "'Quicksand', ui-sans-serif, system-ui, sans-serif", "color": "#eef9fe", "background": "#3b455c", "WebkitFontSmoothing": "antialiased"}} className="landing landing--mobile" ref={root}>
<div className="plr-hero-bg-m" aria-hidden="true" style={{"position": "fixed", "inset": "0", "zIndex": "0"}}></div>
<header className={"plr-in-header plr-header " + v.headerState} style={{"position": "fixed", "top": "0", "left": "0", "right": "0", "zIndex": "20", "boxSizing": "border-box", "display": "flex", "flexDirection": "column"}}>
<div className='plr-header-bg' aria-hidden='true' />
<div style={{"position": "relative", "height": "64px", "boxSizing": "border-box", "padding": "0 16px", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px"}}>
<a href="#plr-m-top" style={{"position": "relative", "display": "flex", "alignItems": "center", "gap": "8px", "minHeight": "44px", "textDecoration": "none", "fontSize": "15px", "fontWeight": "500"}} aria-label="PrimeLendRow home">
<img src="/pictures/landing-logo.png" alt="" width="32" height="26" style={{"display": "block", "width": "32px", "height": "26px"}} />
<span style={{"color": "#ffffff", "fontSize": "15px"}}>Prime<span style={{"color": "var(--landing-accent)"}}>LendRow</span></span>
</a>
<nav aria-label="Primary" style={{"position": "relative", "display": "flex", "alignItems": "center", "gap": "8px"}}>
<Link className="plr-btn" style={{"display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "44px", "padding": "0 18px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "15px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 4px 14px rgba(50, 90, 230, 0.35)"}} to="/auth">Get Started</Link>
<button type="button" aria-label="Toggle accent color" className="plr-icon-btn" style={{"display": "grid", "placeItems": "center", "width": "44px", "height": "44px", "padding": "0", "boxSizing": "border-box", "borderRadius": "12px", "border": "1px solid rgba(150, 170, 225, 0.4)", "background": "#141c35", "color": "#ffffff", "cursor": "pointer", "boxShadow": "0 0 5px rgba(140, 160, 220, 0.35)"}} aria-pressed={v.greenAccent} onClick={v.toggleAccent}>
<svg aria-hidden="true" width="28" height="28" viewBox="1 1 24 24" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"><circle cx="9.5" cy="15.25" r="3.65"></circle><path d="M8.2 14L9.5 15.25"></path><circle cx="9.3" cy="7.3" r="1.6"></circle><circle cx="16.9" cy="10.1" r="2.5"></circle></svg>
</button>
</nav>
</div>
</header>
<section id="plr-m-top" className="plr-screen" style={{"position": "relative", "display": "flex", "flexDirection": "column", "justifyContent": "center", "textAlign": "center", "boxSizing": "border-box", "padding": "88px 24px 64px"}}>
<h1 className="plr-in-h1" style={{"margin": "0", "fontSize": "40px", "fontWeight": "400", "lineHeight": "1.08", "letterSpacing": "0", "color": "#eef9fe"}}><span style={{"color": "var(--landing-accent)"}}>Capital</span> that moves at your <span style={{"color": "var(--landing-accent)"}}>pace.</span></h1>
<p className="plr-in-sub" style={{"margin": "16px 0 0", "fontSize": "15px", "fontWeight": "400", "lineHeight": "26px", "color": "#bbc6d1"}}>KYC, lending, borrowing, repayment, and portfolio insight, unified in one elegant workspace for borrowers and lenders alike.</p>
<div style={{"marginTop": "28px", "display": "grid", "gridTemplateColumns": "repeat(2, minmax(0, 1fr))", "gap": "10px"}}>
<a href="https://kentice.gitbook.io/lendrow" className="plr-in-cta plr-btn" style={{"display": "flex", "alignItems": "center", "justifyContent": "center", "height": "48px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "15px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)"}} target="_blank" rel="noreferrer">Read Docs</a>
<Link className="plr-in-cta2 plr-btn-ghost" style={{"display": "flex", "alignItems": "center", "justifyContent": "center", "height": "48px", "boxSizing": "border-box", "borderRadius": "999px", "border": "1px solid rgba(163, 184, 241, 0.35)", "background": "rgba(20, 28, 53, 0.35)", "color": "#c2cdd7", "fontSize": "15px", "fontWeight": "600", "textDecoration": "none"}} to="/records">View Records</Link>
</div>
<div className={"plr-hint-wrap " + v.hintState} style={{"position": "absolute", "left": "0", "right": "0", "bottom": "20px", "display": "flex", "justifyContent": "center", "pointerEvents": "none"}}>
<a href="#plr-m-products" className="plr-in-hint plr-hint" style={{"pointerEvents": "auto", "display": "flex", "justifyContent": "center", "alignItems": "center", "gap": "8px", "padding": "6px 10px", "fontSize": "15px", "lineHeight": "20px", "whiteSpace": "nowrap", "color": "#a5b0bb", "textDecoration": "none", "transition": "color 220ms ease"}}>
<svg aria-hidden="true" width="8" height="12" viewBox="0 0 7 11" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round"><path d="M3.5 0.5V10M0.8 7.3L3.5 10L6.2 7.3"></path></svg>
<span>Scroll down to see more</span>
</a>
</div>
</section><main style={{"position": "relative"}}>
<section id="plr-m-products" className={"plr-view " + v.productsState} style={{"position": "relative", "padding": "24px 24px 56px", "scrollMarginTop": "64px"}}>
<section className="plr-v plr-v-card plr-card" aria-label="Loan products" style={{"position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "22px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "backgroundClip": "padding-box", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "16px", "color": "#eef9fe"}}>
<div className="plr-v plr-v-reveal" style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "animationDelay": "160ms"}}>
<span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>Loan products</span>
<span style={{"fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#c5cfdb", "padding": "4px 10px", "borderRadius": "999px", "border": "1px solid rgba(190, 207, 245, 0.2)", "background": "rgba(163, 184, 241, 0.06)"}}>Stellar-native</span>
</div>
<h2 className="plr-v plr-v-reveal" style={{"margin": "0", "fontSize": "28px", "fontWeight": "500", "lineHeight": "34px", "color": "#eef9fe", "animationDelay": "230ms"}}>Three ways to borrow</h2>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "300ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="3" width="12" height="9.5" rx="2"></rect><circle cx="8" cy="7.75" r="2.25"></circle><path d="M4.5 12.5V14M11.5 12.5V14"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Deposit-backed</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Secured by your own deposits</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "370ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><ellipse cx="8" cy="4.5" rx="5" ry="2"></ellipse><path d="M3 4.5V8c0 1.1 2.2 2 5 2s5-.9 5-2V4.5"></path><path d="M3 8v3.5c0 1.1 2.2 2 5 2s5-.9 5-2V8"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>XLM collateral-backed</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>XLM worth 120% of the loan</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "440ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><circle cx="6" cy="5" r="2.5"></circle><path d="M1.5 13.5c0-2.5 2-4 4.5-4s4.5 1.5 4.5 4"></path><path d="M10.5 7.5l1.5 1.5 3-3"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Guarantor-backed</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Doubles your limit at every tier</span></div>
</li>
</ul>
<div className="plr-v plr-v-reveal" style={{"paddingTop": "16px", "borderTop": "1px solid rgba(190, 207, 245, 0.14)", "display": "flex", "flexDirection": "column", "gap": "10px", "animationDelay": "510ms"}}>
<div style={{"display": "flex", "alignItems": "baseline", "justifyContent": "space-between", "gap": "12px"}}><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Credit score</span><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>50–150 scale</span></div>
<div aria-hidden="true" style={{"position": "relative", "height": "5px", "borderRadius": "999px", "background": "rgba(255, 255, 255, 0.08)", "overflow": "hidden"}}><div className="plr-v plr-v-bar" style={{"position": "absolute", "inset": "0", "borderRadius": "inherit", "background": "linear-gradient(90deg, rgba(163, 184, 241, 0.35) 0%, var(--landing-accent) 100%)", "animationDelay": "580ms"}}></div></div>
<div aria-hidden="true" style={{"display": "flex", "justifyContent": "space-between", "fontSize": "15px", "lineHeight": "20px", "color": "#b7c1cc"}}><span>50</span><span>100</span><span>150</span></div>
</div>
</section>
</section>
<section id="plr-m-verification" className={"plr-view " + v.verifState} aria-labelledby="plr-m-verif-title" style={{"position": "relative", "padding": "48px 24px 72px", "scrollMarginTop": "64px"}}>
<div style={{"display": "flex", "flexDirection": "column", "textAlign": "center"}}>
<h2 id="plr-m-verif-title" className="plr-v plr-v-rise" style={{"margin": "0", "fontSize": "40px", "fontWeight": "400", "lineHeight": "1.1", "letterSpacing": "0", "color": "#eef9fe", "animationDuration": "var(--plr-d-head)"}}>Verified in <span style={{"color": "var(--landing-accent)"}}>minutes,</span> not <span style={{"color": "var(--landing-accent)"}}>days.</span></h2>
<p className="plr-v plr-v-rise" style={{"margin": "14px 0 0", "fontSize": "15px", "fontWeight": "400", "lineHeight": "26px", "color": "#bbc6d1", "animationDelay": "90ms"}}>Five guided steps pair OCR, live document scanning, and selfie liveness, so lenders trust exactly who they fund, without the paperwork.</p>
<Link className="plr-v plr-v-rise plr-btn" style={{"marginTop": "26px", "display": "flex", "alignItems": "center", "justifyContent": "center", "height": "48px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "15px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)", "animationDelay": "180ms", "animationDuration": "var(--plr-d-reveal)"}} to="/verification">Start Verification</Link>
</div>
<div id="plr-m-vblocks" className="plr-vblocks">
<div className="plr-vrail-line" aria-hidden="true"><span className="plr-vrail-fill"></span></div>
<div className={"plr-vblk " + v.vb.b0} aria-current={v.vcur.b0}>
<span className={"plr-vnode " + v.vn.n0} aria-hidden="true"></span>
<span className="plr-vnum">01 / 05</span>
<h3 className="plr-vh">Upload your ID</h3>
<p className="plr-vp">Add a clear photo of a government ID.</p>
<div className="plr-vtags"><span>Photo or file</span><span>JPG · PNG · WEBP</span></div>
<div className="plr-vmvis" aria-hidden="true"><div className="plr-vm-id"><div className="plr-vm-id-top">Identity card</div><div className="plr-vm-id-body"><span className="ph"></span><span className="tx"><b>DELA CRUZ</b><b>JUAN</b><b>01 JAN 1990</b></span></div><span className="plr-vm-scan"></span></div></div>
</div>
<div className={"plr-vblk " + v.vb.b1} aria-current={v.vcur.b1}>
<span className={"plr-vnode " + v.vn.n1} aria-hidden="true"></span>
<span className="plr-vnum">02 / 05</span>
<h3 className="plr-vh">Live document scan</h3>
<p className="plr-vp">A quick scan reads and cross-checks your details.</p>
<div className="plr-vtags"><span>Real-time check</span><span>Filled in for you</span></div>
<div className="plr-vmvis" aria-hidden="true"><div className="plr-vm-rows"><div className="plr-vm-row"><span>Juan Dela Cruz</span><em>Read</em></div><div className="plr-vm-row"><span>01 / 01 / 1990</span><em>Read</em></div><div className="plr-vm-row"><span>•••• 3456</span><em>Read</em></div></div></div>
</div>
<div className={"plr-vblk " + v.vb.b2} aria-current={v.vcur.b2}>
<span className={"plr-vnode " + v.vn.n2} aria-hidden="true"></span>
<span className="plr-vnum">03 / 05</span>
<h3 className="plr-vh">Selfie liveness</h3>
<p className="plr-vp">Take a live selfie and blink once.</p>
<div className="plr-vtags"><span>One blink</span><span>Live camera only</span></div>
<div className="plr-vmvis" aria-hidden="true"><div className="plr-vm-cam"><svg viewBox="0 0 220 236" width="200" height="214" aria-hidden="true"><path className="b" d="M40 236c6-44 34-62 70-62s64 18 70 62z"></path><ellipse className="f" cx="110" cy="108" rx="38" ry="46"></ellipse><path className="h" d="M72 98c0-30 16-45 38-45s38 15 38 45c-6-15-21-23-38-23s-32 8-38 23z"></path><ellipse className="r" cx="110" cy="110" rx="62" ry="78"></ellipse></svg><span className="plr-vm-pill">Blink once</span></div></div>
</div>
<div className={"plr-vblk " + v.vb.b3} aria-current={v.vcur.b3}>
<span className={"plr-vnode " + v.vn.n3} aria-hidden="true"></span>
<span className="plr-vnum">04 / 05</span>
<h3 className="plr-vh">Link your wallet</h3>
<p className="plr-vp">Connect your Varial wallet with a signature.</p>
<div className="plr-vtags"><span>Signature, not a transaction</span></div>
<div className="plr-vmvis" aria-hidden="true"><div className="plr-vm-wal"><span className="plr-vwt pri">V</span><span className="tx"><b>Varial</b><span>GDX4…7KP</span></span><em>Connected</em></div></div>
</div>
<div className={"plr-vblk " + v.vb.b4} aria-current={v.vcur.b4}>
<span className={"plr-vnode " + v.vn.n4} aria-hidden="true"></span>
<span className="plr-vnum">05 / 05</span>
<h3 className="plr-vh">Review &amp; submit</h3>
<p className="plr-vp">Check your details, then sign and submit.</p>
<div className="plr-vtags"><span>Verified after review</span></div>
<div className="plr-vmvis" aria-hidden="true"><div className="plr-vm-stat"><span>Submitted<em className="a">Done</em></span><span>Review<em className="b">In progress</em></span><span className="m">Verified<em>Next</em></span></div></div>
</div>
<div className="plr-vrail-dotw" aria-hidden="true"><span className="plr-vrail-dot"></span></div>
</div>
</section>
<section id="plr-m-lend" className={"plr-view " + v.msec.lend} aria-labelledby="plr-m-lend-title" style={{"position": "relative", "padding": "48px 24px 72px", "scrollMarginTop": "64px"}}>
<div style={{"display": "flex", "flexDirection": "column", "textAlign": "center"}}>
<h2 id="plr-m-lend-title" className="plr-v plr-v-rise" style={{"margin": "0", "fontSize": "40px", "fontWeight": "400", "lineHeight": "1.1", "letterSpacing": "0", "color": "#eef9fe", "animationDuration": "var(--plr-d-head)"}}>Put idle capital <span style={{"color": "var(--landing-accent)"}}>to work.</span></h2>
<p className="plr-v plr-v-rise" style={{"margin": "14px 0 0", "fontSize": "15px", "fontWeight": "400", "lineHeight": "26px", "color": "#bbc6d1", "animationDelay": "90ms"}}>Deposit with PayPal or Stripe. Money that isn’t being used can be withdrawn anytime.</p>
<Link className="plr-v plr-v-rise plr-btn" style={{"marginTop": "26px", "display": "flex", "alignItems": "center", "justifyContent": "center", "height": "48px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "15px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)", "animationDelay": "180ms", "animationDuration": "var(--plr-d-reveal)"}} to="/lending">Start Lending</Link>
</div>
<div className="plr-v plr-v-card plr-card" style={{"marginTop": "32px", "position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "22px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "backgroundClip": "padding-box", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "16px", "color": "#eef9fe", "animationDelay": "240ms"}}>
<div className="plr-v plr-v-reveal" style={{"display": "flex", "flexWrap": "wrap", "alignItems": "center", "justifyContent": "space-between", "gap": "8px 12px", "animationDelay": "400ms"}}>
<span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>Lending</span>
<span style={{"fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#c5cfdb", "padding": "4px 10px", "borderRadius": "999px", "border": "1px solid rgba(190, 207, 245, 0.2)", "background": "rgba(163, 184, 241, 0.06)"}}>Pooled deposits</span>
</div>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "470ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><path d="M8 2l6 3-6 3-6-3z"></path><path d="M2 8.25l6 3 6-3"></path><path d="M2 11.25l6 3 6-3"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Deposit</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Add money to the pool with PayPal or Stripe.</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "540ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="7" width="10" height="7" rx="1.5"></rect><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Locked while in use</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Money that’s loaned out, backing a loan, or pledged stays put.</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "610ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><path d="M8 10V2.5"></path><path d="M5 5.5l3-3 3 3"></path><path d="M2.5 9.5v2.5a1.5 1.5 0 0 0 1.5 1.5h8a1.5 1.5 0 0 0 1.5-1.5V9.5"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Withdraw the rest</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Anything not in use is yours to withdraw.</span></div>
</li>
</ul>
<p className="plr-v plr-v-reveal" style={{"margin": "0", "paddingTop": "16px", "borderTop": "1px solid rgba(190, 207, 245, 0.14)", "display": "flex", "alignItems": "flex-start", "gap": "8px", "fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb", "animationDelay": "680ms"}}><svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 16px", "marginTop": "2px"}}><path d="M3 2.5h8.5A1.5 1.5 0 0 1 13 4v9.5H4.5A1.5 1.5 0 0 1 3 12z"></path><path d="M5.5 5.5h5M5.5 8h5"></path></svg>Every movement is recorded with double-entry accounting.</p>
</div>
</section>
<section id="plr-m-borrow" className={"plr-view " + v.msec.borrow} aria-labelledby="plr-m-borrow-title" style={{"position": "relative", "padding": "48px 24px 72px", "scrollMarginTop": "64px"}}>
<div style={{"display": "flex", "flexDirection": "column", "textAlign": "center"}}>
<h2 id="plr-m-borrow-title" className="plr-v plr-v-rise" style={{"margin": "0", "fontSize": "40px", "fontWeight": "400", "lineHeight": "1.1", "letterSpacing": "0", "color": "#eef9fe", "animationDuration": "var(--plr-d-head)"}}>Borrow against <span style={{"color": "var(--landing-accent)"}}>what you have.</span></h2>
<p className="plr-v plr-v-rise" style={{"margin": "14px 0 0", "fontSize": "15px", "fontWeight": "400", "lineHeight": "26px", "color": "#bbc6d1", "animationDelay": "90ms"}}>Borrow against your deposit, with XLM collateral, or with a guarantor’s backing.</p>
<Link className="plr-v plr-v-rise plr-btn" style={{"marginTop": "26px", "display": "flex", "alignItems": "center", "justifyContent": "center", "height": "48px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "15px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)", "animationDelay": "180ms", "animationDuration": "var(--plr-d-reveal)"}} to="/borrow">Start Borrowing</Link>
</div>
<div className="plr-v plr-v-card plr-card" style={{"marginTop": "32px", "position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "22px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "backgroundClip": "padding-box", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "16px", "color": "#eef9fe", "animationDelay": "240ms"}}>
<div className="plr-v plr-v-reveal" style={{"display": "flex", "flexWrap": "wrap", "alignItems": "center", "justifyContent": "space-between", "gap": "8px 12px", "animationDelay": "400ms"}}>
<span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>How borrowing works</span>
</div>
<ol style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "470ms"}}>
<span className="plr-row-icon" aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": "15px", "fontWeight": "700", "lineHeight": "20px", "color": "var(--landing-accent)"}}>01</span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Choose your backing</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Your deposit, XLM worth 120% of the loan, or a guarantor.</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "540ms"}}>
<span className="plr-row-icon" aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": "15px", "fontWeight": "700", "lineHeight": "20px", "color": "var(--landing-accent)"}}>02</span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>See your limit</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Your tier sets your limit. A guarantor doubles it.</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "610ms"}}>
<span className="plr-row-icon" aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": "15px", "fontWeight": "700", "lineHeight": "20px", "color": "var(--landing-accent)"}}>03</span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Get your loan</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Borrow any amount up to your limit.</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "680ms"}}>
<span className="plr-row-icon" aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": "15px", "fontWeight": "700", "lineHeight": "20px", "color": "var(--landing-accent)"}}>04</span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Repay and grow your score</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Your 50–150 score updates at the end of each term.</span></div>
</li>
</ol>
</div>
</section>
<section id="plr-m-pay" className={"plr-view " + v.msec.pay} aria-labelledby="plr-m-pay-title" style={{"position": "relative", "padding": "48px 24px 72px", "scrollMarginTop": "64px"}}>
<div style={{"display": "flex", "flexDirection": "column", "textAlign": "center"}}>
<h2 id="plr-m-pay-title" className="plr-v plr-v-rise" style={{"margin": "0", "fontSize": "40px", "fontWeight": "400", "lineHeight": "1.1", "letterSpacing": "0", "color": "#eef9fe", "animationDuration": "var(--plr-d-head)"}}>Repayments, <span style={{"color": "var(--landing-accent)"}}>kept simple.</span></h2>
<p className="plr-v plr-v-rise" style={{"margin": "14px 0 0", "fontSize": "15px", "fontWeight": "400", "lineHeight": "26px", "color": "#bbc6d1", "animationDelay": "90ms"}}>Pay with PayPal or Stripe. Interest is charged only on what you still owe.</p>
<Link className="plr-v plr-v-rise plr-btn" style={{"marginTop": "26px", "display": "flex", "alignItems": "center", "justifyContent": "center", "height": "48px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "15px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)", "animationDelay": "180ms", "animationDuration": "var(--plr-d-reveal)"}} to="/records">View Records</Link>
</div>
<div className="plr-v plr-v-card plr-card" style={{"marginTop": "32px", "position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "22px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "backgroundClip": "padding-box", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "16px", "color": "#eef9fe", "animationDelay": "240ms"}}>
<div className="plr-v plr-v-reveal" style={{"display": "flex", "flexWrap": "wrap", "alignItems": "center", "justifyContent": "space-between", "gap": "8px 12px", "animationDelay": "400ms"}}>
<span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>Repayments</span>
<span style={{"fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#c5cfdb", "padding": "4px 10px", "borderRadius": "999px", "border": "1px solid rgba(190, 207, 245, 0.2)", "background": "rgba(163, 184, 241, 0.06)"}}>PayPal · Stripe</span>
</div>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "470ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><rect x="1.5" y="3.5" width="13" height="9" rx="1.75"></rect><path d="M1.5 6.5h13M4 10h2.5"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Pay with PayPal or Stripe</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>No wallet to connect.</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "540ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><ellipse cx="8" cy="4.5" rx="5" ry="2"></ellipse><path d="M3 4.5V8c0 1.1 2.2 2 5 2s5-.9 5-2V4.5"></path><path d="M3 8v3.5c0 1.1 2.2 2 5 2s5-.9 5-2V8"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Deposit or XLM-backed loans</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>2% to 1% interest per month.</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "610ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><circle cx="6" cy="5" r="2.5"></circle><path d="M1.5 13.5c0-2.5 2-4 4.5-4s4.5 1.5 4.5 4"></path><path d="M10.5 7.5l1.5 1.5 3-3"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Guarantor-backed loans</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>6% to 3% interest per month.</span></div>
</li>
<li className="plr-v plr-v-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "680ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><path d="M2 4l4 4 3-3 5 6"></path><path d="M10.5 11H14V7.5"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Interest shrinks as you pay</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>It’s charged on your remaining principal.</span></div>
</li>
</ul>
</div>
</section>
<section id="plr-m-interest" className={"plr-view " + v.msec.interest} aria-labelledby="plr-m-interest-title" style={{"position": "relative", "padding": "48px 24px 72px", "scrollMarginTop": "64px"}}>
<div style={{"display": "flex", "flexDirection": "column", "textAlign": "center"}}>
<h2 id="plr-m-interest-title" className="plr-v plr-v-rise" style={{"margin": "0", "fontSize": "40px", "fontWeight": "400", "lineHeight": "1.1", "letterSpacing": "0", "color": "#eef9fe", "animationDuration": "var(--plr-d-head)"}}>Where interest <span style={{"color": "var(--landing-accent)"}}>goes.</span></h2>
<p className="plr-v plr-v-rise" style={{"margin": "14px 0 0", "fontSize": "15px", "fontWeight": "400", "lineHeight": "26px", "color": "#bbc6d1", "animationDelay": "90ms"}}>Each interest payment is split the same way. Depositors earn mostly from XLM-backed loans, on the part of their money that was lent.</p>
<a href="https://kentice.gitbook.io/lendrow" className="plr-v plr-v-rise plr-btn" style={{"marginTop": "26px", "display": "flex", "alignItems": "center", "justifyContent": "center", "height": "48px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "15px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)", "animationDelay": "180ms", "animationDuration": "var(--plr-d-reveal)"}} target="_blank" rel="noreferrer">Read Docs</a>
</div>
<div className="plr-v plr-v-card plr-card" style={{"marginTop": "32px", "position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "22px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "backgroundClip": "padding-box", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "18px", "color": "#eef9fe", "animationDelay": "240ms"}}>
<div className="plr-v plr-v-reveal" style={{"display": "flex", "flexWrap": "wrap", "alignItems": "center", "justifyContent": "space-between", "gap": "8px 12px", "animationDelay": "400ms"}}>
<span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>Interest split</span>
<span style={{"fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#c5cfdb", "padding": "4px 10px", "borderRadius": "999px", "border": "1px solid rgba(190, 207, 245, 0.2)", "background": "rgba(163, 184, 241, 0.06)"}}>Every payment</span>
</div>
<div className="plr-v plr-v-reveal" aria-hidden="true" style={{"display": "flex", "gap": "3px", "height": "8px", "animationDelay": "450ms"}}><span style={{"flex": "40 1 0", "borderRadius": "999px", "background": "#5b8def"}}></span><span style={{"flex": "20 1 0", "borderRadius": "999px", "background": "#e27a3f"}}></span><span style={{"flex": "10 1 0", "borderRadius": "999px", "background": "#43b07a"}}></span><span style={{"flex": "30 1 0", "borderRadius": "999px", "background": "#e05a87"}}></span></div>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column"}}>
<li className="plr-v plr-v-reveal" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "animationDelay": "500ms"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#5b8def"}}></span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Depositors</span><span style={{"fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb"}}>Mostly from XLM-backed loans, shared by how much of each one’s money was lent.</span></span>
<span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>40%</span>
</li>
<li className="plr-v plr-v-reveal" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "borderTop": "1px solid rgba(190, 207, 245, 0.1)", "animationDelay": "560ms"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#e27a3f"}}></span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Lending reserve</span></span>
<span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>20%</span>
</li>
<li className="plr-v plr-v-reveal" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "borderTop": "1px solid rgba(190, 207, 245, 0.1)", "animationDelay": "620ms"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#43b07a"}}></span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Platform</span></span>
<span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>10%</span>
</li>
<li className="plr-v plr-v-reveal" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "borderTop": "1px solid rgba(190, 207, 245, 0.1)", "animationDelay": "680ms"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#e05a87"}}></span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}><span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>Recovery fund</span></span>
<span style={{"fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "color": "#eef9fe"}}>30%</span>
</li>
</ul>
<p className="plr-v plr-v-reveal" style={{"margin": "0", "paddingTop": "14px", "borderTop": "1px solid rgba(190, 207, 245, 0.14)", "display": "flex", "alignItems": "flex-start", "gap": "8px", "fontSize": "15px", "lineHeight": "20px", "color": "#c5cfdb", "animationDelay": "760ms"}}><svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 16px", "marginTop": "2px"}}><path d="M6 7.2a2.4 2.4 0 1 0 0-4.8 2.4 2.4 0 0 0 0 4.8z"></path><path d="M1.5 13.5c0-2.4 2-4 4.5-4s4.5 1.6 4.5 4"></path><path d="M11 3.2a2.2 2.2 0 0 1 0 4.2M12.5 9.7c1.2.5 2 1.6 2 3.3"></path></svg>With guarantors (up to 2), they share 10–25% pro rata, taken from the recovery fund.</p>
</div>
</section>
<section id="plr-m-faq" className={"plr-view " + v.msec.faq} aria-labelledby="plr-m-faq-title" style={{"position": "relative", "padding": "48px 24px 72px", "scrollMarginTop": "64px"}}>
<div style={{"display": "flex", "flexDirection": "column", "textAlign": "center"}}>
<h2 id="plr-m-faq-title" className="plr-v plr-v-rise" style={{"margin": "0", "fontSize": "40px", "fontWeight": "400", "lineHeight": "1.1", "letterSpacing": "0", "color": "#eef9fe", "animationDuration": "var(--plr-d-head)"}}>Questions, <span style={{"color": "var(--landing-accent)"}}>answered.</span></h2>
<p className="plr-v plr-v-rise" style={{"margin": "14px 0 0", "fontSize": "15px", "fontWeight": "400", "lineHeight": "26px", "color": "#bbc6d1", "animationDelay": "90ms"}}>Short answers to what members ask most. The docs cover the details.</p>
</div>
<div className="plr-v plr-v-card plr-card" style={{"marginTop": "32px", "position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "14px 16px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "backgroundClip": "padding-box", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "color": "#eef9fe", "animationDelay": "180ms"}}>
<div className="plr-v plr-v-reveal" style={{"animationDelay": "340ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-m-faq-q1" className="plr-faq-q" aria-expanded={v.faq.o1} aria-controls="plr-m-faq-a1" onClick={v.onFaq1} style={{"width": "100%", "minHeight": "52px", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 4px", "border": "0", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "textAlign": "left", "cursor": "pointer"}}>Who can join PrimeLendRow?<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 16px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-m-faq-a1" className={"plr-faq-a " + v.faq.c1} role="region" aria-labelledby="plr-m-faq-q1"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 4px 16px", "fontSize": "15px", "lineHeight": "22px", "color": "#c5cfdb"}}>Membership is KYC-gated. Complete the five verification steps once to borrow or lend.</p></div></div>
</div>
<div className="plr-v plr-v-reveal" style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)", "animationDelay": "410ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-m-faq-q2" className="plr-faq-q" aria-expanded={v.faq.o2} aria-controls="plr-m-faq-a2" onClick={v.onFaq2} style={{"width": "100%", "minHeight": "52px", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 4px", "border": "0", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "textAlign": "left", "cursor": "pointer"}}>What can I borrow against?<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 16px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-m-faq-a2" className={"plr-faq-a " + v.faq.c2} role="region" aria-labelledby="plr-m-faq-q2"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 4px 16px", "fontSize": "15px", "lineHeight": "22px", "color": "#c5cfdb"}}>Your deposit, XLM worth 120% of the loan, or a guarantor, who doubles your limit at every tier.</p></div></div>
</div>
<div className="plr-v plr-v-reveal" style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)", "animationDelay": "480ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-m-faq-q3" className="plr-faq-q" aria-expanded={v.faq.o3} aria-controls="plr-m-faq-a3" onClick={v.onFaq3} style={{"width": "100%", "minHeight": "52px", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 4px", "border": "0", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "textAlign": "left", "cursor": "pointer"}}>How does my credit score change?<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 16px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-m-faq-a3" className={"plr-faq-a " + v.faq.c3} role="region" aria-labelledby="plr-m-faq-q3"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 4px 16px", "fontSize": "15px", "lineHeight": "22px", "color": "#c5cfdb"}}>Scores run from 50 to 150 and only update at the end of each loan term.</p></div></div>
</div>
<div className="plr-v plr-v-reveal" style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)", "animationDelay": "550ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-m-faq-q4" className="plr-faq-q" aria-expanded={v.faq.o4} aria-controls="plr-m-faq-a4" onClick={v.onFaq4} style={{"width": "100%", "minHeight": "52px", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 4px", "border": "0", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "textAlign": "left", "cursor": "pointer"}}>How do repayments work?<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 16px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-m-faq-a4" className={"plr-faq-a " + v.faq.c4} role="region" aria-labelledby="plr-m-faq-q4"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 4px 16px", "fontSize": "15px", "lineHeight": "22px", "color": "#c5cfdb"}}>Pay with PayPal or Stripe. Interest is charged on your remaining principal, so it drops as you pay.</p></div></div>
</div>
<div className="plr-v plr-v-reveal" style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)", "animationDelay": "620ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-m-faq-q5" className="plr-faq-q" aria-expanded={v.faq.o5} aria-controls="plr-m-faq-a5" onClick={v.onFaq5} style={{"width": "100%", "minHeight": "52px", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 4px", "border": "0", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "18px", "fontWeight": "600", "lineHeight": "24px", "textAlign": "left", "cursor": "pointer"}}>How do lenders earn?<svg aria-hidden="true" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 16px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-m-faq-a5" className={"plr-faq-a " + v.faq.c5} role="region" aria-labelledby="plr-m-faq-q5"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 4px 16px", "fontSize": "15px", "lineHeight": "22px", "color": "#c5cfdb"}}>Depositors get 40% of interest, mostly from XLM-backed loans, shared by how much of each one’s money was lent.</p></div></div>
</div>
</div>
<a href="https://kentice.gitbook.io/lendrow" className="plr-v plr-v-rise plr-btn" style={{"marginTop": "24px", "display": "flex", "alignItems": "center", "justifyContent": "center", "height": "48px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "15px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)", "animationDelay": "700ms", "animationDuration": "var(--plr-d-reveal)"}} target="_blank" rel="noreferrer">Read Docs</a>
</section>
</main>
<footer className={"plr-footer " + v.footerState} style={{"position": "relative", "zIndex": "1", "height": "64px", "boxSizing": "border-box", "padding": "0 16px", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "borderTop": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-chrome-filter)", "WebkitBackdropFilter": "var(--landing-glass-chrome-filter)"}}>
<p style={{"margin": "0", "fontSize": "15px", "fontWeight": "400", "lineHeight": "20px", "color": "#95a3b4"}}>© 2026 PrimeLendRow.<br />All rights reserved.</p>
<nav aria-label="Footer" style={{"display": "flex", "alignItems": "center", "gap": "16px"}}>
<a href="https://kentice.gitbook.io/lendrow" className="plr-link" style={{"display": "inline-flex", "alignItems": "center", "minHeight": "44px", "fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#c2cdd7", "textDecoration": "none"}} target="_blank" rel="noreferrer">Docs</a>
<a href="#terms" className="plr-link" style={{"display": "inline-flex", "alignItems": "center", "minHeight": "44px", "fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#c2cdd7", "textDecoration": "none"}} onClick={v.onTerms} aria-haspopup="dialog">Terms</a>
<a href="#privacy" className="plr-link" style={{"display": "inline-flex", "alignItems": "center", "minHeight": "44px", "fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#c2cdd7", "textDecoration": "none"}} onClick={v.onPrivacy} aria-haspopup="dialog">Privacy</a>
</nav>
</footer>
</div>
    )
}
