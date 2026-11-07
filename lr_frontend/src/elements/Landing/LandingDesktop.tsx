import type { CSSProperties, RefObject } from 'react'
import { Link } from 'react-router-dom'
import type { LandingValues } from './useLandingPresentation'

type Props = { v: LandingValues; root: RefObject<HTMLDivElement | null> }

// The supplied design has distinct desktop and mobile compositions.
export default function LandingDesktop({ v, root }: Props) {
    return (
<div style={{"minHeight": "100vh", "overflow": "clip", "fontFamily": "'Quicksand', ui-sans-serif, system-ui, sans-serif", "color": "#eef9fe", "background": "#3b455c", "WebkitFontSmoothing": "antialiased"}} className="landing landing--desktop" ref={root}>
<div className="plr-hero-bg" aria-hidden="true" style={{"position": "fixed", "inset": "0", "zIndex": "0"}}></div>
<header className={"plr-in-header plr-header " + v.headerState} style={{"position": "fixed", "top": "0", "left": "0", "right": "0", "zIndex": "20", "minHeight": "58px", "boxSizing": "border-box", "padding": "0 24px 0 26.5px", "display": "flex", "flexWrap": "wrap", "alignItems": "center", "justifyContent": "space-between", "columnGap": "16px"}}>
<div className='plr-header-bg' aria-hidden='true' />
<a href="#plr-top" style={{"position": "relative", "display": "flex", "alignItems": "center", "gap": "8px", "textDecoration": "none", "fontSize": "16px", "fontWeight": "500", "letterSpacing": "0"}} aria-label="PrimeLendRow home">
<img src="/pictures/landing-logo.png" alt="" width="34" height="27.5" style={{"display": "block", "width": "34px", "height": "27.5px"}} />
<span style={{"color": "#ffffff", "fontSize": "16px"}}>Prime<span style={{"color": "var(--landing-accent)"}}>LendRow</span></span>
</a>
<nav aria-label="Sections" className="plr-sections" style={{"position": "absolute", "left": "50%", "top": "0", "bottom": "0", "translate": "-50% 0", "display": "flex", "alignItems": "center", "gap": "2px"}}>
<a href="#plr-verification" className={"plr-navlink " + v.nav.verify} onClick={v.onNav} aria-current={v.cur.verify} style={{"padding": "7px 12px", "borderRadius": "999px", "fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c2cdd7", "textDecoration": "none"}}>Verify</a>
<a href="#plr-lend" className={"plr-navlink " + v.nav.lend} onClick={v.onNav} aria-current={v.cur.lend} style={{"padding": "7px 12px", "borderRadius": "999px", "fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c2cdd7", "textDecoration": "none"}}>Lend</a>
<a href="#plr-borrow" className={"plr-navlink " + v.nav.borrow} onClick={v.onNav} aria-current={v.cur.borrow} style={{"padding": "7px 12px", "borderRadius": "999px", "fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c2cdd7", "textDecoration": "none"}}>Borrow</a>
<a href="#plr-pay" className={"plr-navlink " + v.nav.pay} onClick={v.onNav} aria-current={v.cur.pay} style={{"padding": "7px 12px", "borderRadius": "999px", "fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c2cdd7", "textDecoration": "none"}}>Pay</a>
<a href="#plr-interest" className={"plr-navlink " + v.nav.interest} onClick={v.onNav} aria-current={v.cur.interest} style={{"padding": "7px 12px", "borderRadius": "999px", "fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c2cdd7", "textDecoration": "none"}}>Interest</a>
<a href="#plr-faq" className={"plr-navlink " + v.nav.faq} onClick={v.onNav} aria-current={v.cur.faq} style={{"padding": "7px 12px", "borderRadius": "999px", "fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c2cdd7", "textDecoration": "none"}}>FAQ</a>
</nav>
<nav aria-label="Primary" style={{"position": "relative", "display": "flex", "alignItems": "center", "gap": "10px"}}>
<Link className="plr-btn" style={{"display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "27px", "padding": "0 15px", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "12px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 4px 14px rgba(50, 90, 230, 0.35)"}} to="/auth">Get Started</Link>
<button type="button" aria-label="Toggle accent color" className="plr-icon-btn" style={{"display": "grid", "placeItems": "center", "width": "26px", "height": "26px", "padding": "0", "boxSizing": "border-box", "borderRadius": "7px", "border": "1px solid rgba(150, 170, 225, 0.4)", "background": "#141c35", "color": "#ffffff", "cursor": "pointer", "boxShadow": "0 0 4px rgba(140, 160, 220, 0.35)"}} aria-pressed={v.greenAccent} onClick={v.toggleAccent}>
<svg aria-hidden="true" width="24" height="24" viewBox="1 1 24 24" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round"><circle cx="9.5" cy="15.25" r="3.65"></circle><path d="M8.2 14L9.5 15.25"></path><circle cx="9.3" cy="7.3" r="1.6"></circle><circle cx="16.9" cy="10.1" r="2.5"></circle></svg>
</button>
</nav>
</header>
<main style={{"position": "relative"}}>
<section id="plr-top" className="plr-screen plr-split" style={{"position": "relative", "display": "grid", "gridTemplateColumns": "repeat(2, minmax(0, 1fr))", "alignItems": "center", "columnGap": "48px", "rowGap": "48px", "boxSizing": "border-box", "padding": "142px clamp(20px, 4.95vw, 95px) 72px"}}>
<div style={{"position": "relative", "minWidth": "0", "display": "flex", "flexDirection": "column", "alignItems": "flex-start"}}>
<h1 className="plr-in-h1" style={{"margin": "0", "maxWidth": "620px", "fontSize": "56px", "fontWeight": "400", "lineHeight": "1.08", "letterSpacing": "0", "color": "#eef9fe"}}><span style={{"color": "var(--landing-accent)"}}>Capital</span> that moves at your <span style={{"color": "var(--landing-accent)"}}>pace.</span></h1>
<p className="plr-in-sub" style={{"margin": "15px 0 0", "maxWidth": "400px", "fontSize": "15px", "fontWeight": "400", "lineHeight": "20px", "color": "#bbc6d1"}}>KYC, lending, borrowing, repayment, and portfolio insight, unified in one elegant workspace for borrowers and lenders alike.</p>
<div style={{"marginTop": "26px", "display": "flex", "flexWrap": "wrap", "gap": "10px"}}>
<a href="https://kentice.gitbook.io/lendrow" className="plr-in-cta plr-btn" style={{"display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "33px", "padding": "0 22px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "12px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)"}} target="_blank" rel="noreferrer">Read Docs</a>
<Link className="plr-in-cta2 plr-btn-ghost" style={{"display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "33px", "padding": "0 22px", "boxSizing": "border-box", "borderRadius": "999px", "border": "1px solid rgba(163, 184, 241, 0.35)", "background": "rgba(20, 28, 53, 0.35)", "color": "#c2cdd7", "fontSize": "12px", "fontWeight": "600", "textDecoration": "none"}} to="/records">View Records</Link>
</div>
</div>
<div className="plr-card-wrap" style={{"position": "relative", "width": "416px", "maxWidth": "100%", "justifySelf": "center"}}>
<section className="plr-card" aria-label="Loan products" style={{"position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "28px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backgroundClip": "padding-box", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "18px", "color": "#eef9fe"}}>
<div className="plr-reveal" style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "animationDelay": "520ms"}}>
<span style={{"fontSize": "12px", "fontWeight": "600", "lineHeight": "16px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>Loan products</span>
<span style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c5cfdb", "padding": "4px 10px", "borderRadius": "999px", "border": "1px solid rgba(190, 207, 245, 0.2)", "background": "rgba(163, 184, 241, 0.06)"}}>Stellar-native</span>
</div>
<h2 className="plr-reveal" style={{"margin": "0", "fontSize": "23px", "fontWeight": "500", "lineHeight": "28px", "color": "#eef9fe", "animationDelay": "590ms"}}>Three ways to borrow</h2>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li className="plr-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "660ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="3" width="12" height="9.5" rx="2"></rect><circle cx="8" cy="7.75" r="2.25"></circle><path d="M4.5 12.5V14M11.5 12.5V14"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Deposit-backed</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Secured by your own deposits</span></div>
</li>
<li className="plr-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "730ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><ellipse cx="8" cy="4.5" rx="5" ry="2"></ellipse><path d="M3 4.5V8c0 1.1 2.2 2 5 2s5-.9 5-2V4.5"></path><path d="M3 8v3.5c0 1.1 2.2 2 5 2s5-.9 5-2V8"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>XLM collateral-backed</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>XLM worth 120% of the loan</span></div>
</li>
<li className="plr-reveal plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "800ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><circle cx="6" cy="5" r="2.5"></circle><path d="M1.5 13.5c0-2.5 2-4 4.5-4s4.5 1.5 4.5 4"></path><path d="M10.5 7.5l1.5 1.5 3-3"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Guarantor-backed</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Doubles your limit at every tier</span></div>
</li>
</ul>
<div className="plr-reveal" style={{"paddingTop": "16px", "borderTop": "1px solid rgba(190, 207, 245, 0.14)", "display": "flex", "flexDirection": "column", "gap": "10px", "animationDelay": "870ms"}}>
<div style={{"display": "flex", "alignItems": "baseline", "justifyContent": "space-between", "gap": "12px", "fontSize": "15px"}}><span style={{"fontSize": "12px", "lineHeight": "16px", "color": "#c5cfdb"}}>Credit score</span><span style={{"fontSize": "12px", "fontWeight": "600", "lineHeight": "16px", "color": "#eef9fe"}}>50–150 scale</span></div>
<div aria-hidden="true" style={{"position": "relative", "height": "5px", "borderRadius": "999px", "background": "rgba(255, 255, 255, 0.08)", "overflow": "hidden"}}><div className="plr-bar" style={{"position": "absolute", "inset": "0", "borderRadius": "inherit", "background": "linear-gradient(90deg, rgba(163, 184, 241, 0.35) 0%, var(--landing-accent) 100%)"}}></div></div>
<div aria-hidden="true" style={{"display": "flex", "justifyContent": "space-between", "fontSize": "12px", "lineHeight": "16px", "color": "#b7c1cc"}}><span>50</span><span>100</span><span>150</span></div>
</div>
</section>
</div>
<div className={"plr-hint-wrap " + v.hintState} style={{"position": "absolute", "left": "0", "right": "0", "bottom": "22px", "display": "flex", "justifyContent": "center", "pointerEvents": "none"}}>
<a href="#plr-verification" className="plr-in-hint plr-hint" style={{"pointerEvents": "auto", "display": "flex", "justifyContent": "center", "alignItems": "center", "gap": "7px", "padding": "4px 8px", "fontSize": "12px", "lineHeight": "16px", "color": "#a5b0bb", "textDecoration": "none", "transition": "color 220ms ease"}}>
<svg aria-hidden="true" width="7" height="11" viewBox="0 0 7 11" fill="none" stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round"><path d="M3.5 0.5V10M0.8 7.3L3.5 10L6.2 7.3"></path></svg>
<span style={{"fontSize": "12px"}}>Scroll down to see more</span>
</a>
</div>
</section>
<section id="plr-verification" className={"plr-verif plr-vscene " + v.verifState + " " + v.vZoomCls + " " + v.vSceneCls} aria-labelledby="plr-verif-title" style={{"position": "relative", "--acc": "var(--landing-accent)"} as CSSProperties}>
<div className="plr-vgrid" style={{"display": "grid", "gridTemplateColumns": "minmax(0, 5fr) minmax(0, 7fr)", "columnGap": "56px", "boxSizing": "border-box", "padding": "0 clamp(20px, 4.95vw, 95px) 96px"}}>
<div className="plr-vcol-l" style={{"position": "relative", "minWidth": "0"}}>
<div className="plr-vintro">
<h2 id="plr-verif-title" className="plr-v-h2" style={{"margin": "0", "maxWidth": "620px", "fontSize": "56px", "fontWeight": "400", "lineHeight": "1.08", "letterSpacing": "0", "color": "#eef9fe"}}>Verified in <span style={{"color": "var(--landing-accent)"}}>minutes,</span> not <span style={{"color": "var(--landing-accent)"}}>days.</span></h2>
<p className="plr-v-sub" style={{"margin": "14px 0 0", "maxWidth": "420px", "fontSize": "15px", "fontWeight": "400", "lineHeight": "20px", "color": "#bbc6d1"}}>Five guided steps pair OCR, live document scanning, and selfie liveness, so lenders trust exactly who they fund, without the paperwork.</p>
<Link className="plr-v-cta plr-btn" style={{"marginTop": "26px", "display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "33px", "padding": "0 22px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "12px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)"}} to="/verification">Start Verification</Link>
</div>
<div id="plr-vblocks" className="plr-vblocks">
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
</div>
<div className="plr-vcol-r" style={{"position": "relative", "minWidth": "0"}}>
<div className="plr-vstick">
<div id="plr-vstage-area" className="plr-vstage-area">
<ol className="plr-steps" aria-label="Verification steps" style={{"position": "relative", "width": "416px", "maxWidth": "100%", "justifySelf": "center", "margin": "0", "padding": "0", "listStyle": "none", "display": "grid", "gridTemplateColumns": "repeat(2, minmax(0, 1fr))", "gap": "12px"}}>
<li className="plr-v-step plr-step" style={{"boxSizing": "border-box", "minHeight": "144px", "padding": "20px 16px 18px", "borderRadius": "12px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 10px 30px -14px rgba(6, 10, 26, 0.35)", "display": "flex", "flexDirection": "column"}}>
<span style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "letterSpacing": "0.04em", "color": "#95a3b4"}}>01</span>
<svg className="plr-step-icon" aria-hidden="true" width="19" height="19" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" style={{"marginTop": "12px", "display": "block"}}><rect x="0.75" y="2.5" width="14.5" height="11" rx="1.75"></rect><circle cx="5.25" cy="6.6" r="1.5"></circle><path d="M2.9 11.2c0-1.3 1-2.1 2.35-2.1s2.35.8 2.35 2.1"></path><path d="M9.6 6.5h3.4M9.6 9.3h3.4"></path></svg>
<h3 style={{"margin": "8px 0 0", "fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#eef9fe"}}>Upload your ID</h3>
<p style={{"margin": "5px 0 0", "fontSize": "12px", "fontWeight": "300", "lineHeight": "16px", "color": "#a6b4c0"}}>Submit a government ID and OCR extracts your details instantly.</p>
</li>
<li className="plr-v-step plr-step" style={{"boxSizing": "border-box", "minHeight": "144px", "padding": "20px 16px 18px", "borderRadius": "12px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 10px 30px -14px rgba(6, 10, 26, 0.35)", "display": "flex", "flexDirection": "column"}}>
<span style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "letterSpacing": "0.04em", "color": "#95a3b4"}}>02</span>
<svg className="plr-step-icon" aria-hidden="true" width="19" height="19" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" style={{"marginTop": "12px", "display": "block"}}><path d="M1.5 5V3A1.5 1.5 0 0 1 3 1.5H5M11 1.5h2A1.5 1.5 0 0 1 14.5 3v2M14.5 11v2a1.5 1.5 0 0 1-1.5 1.5h-2M5 14.5H3A1.5 1.5 0 0 1 1.5 13v-2"></path></svg>
<h3 style={{"margin": "8px 0 0", "fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#eef9fe"}}>Live document scan</h3>
<p style={{"margin": "5px 0 0", "fontSize": "12px", "fontWeight": "300", "lineHeight": "16px", "color": "#a6b4c0"}}>A quick id scan cross-checks your document in real time.</p>
</li>
<li className="plr-v-step plr-step" style={{"boxSizing": "border-box", "minHeight": "144px", "padding": "20px 16px 18px", "borderRadius": "12px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 10px 30px -14px rgba(6, 10, 26, 0.35)", "display": "flex", "flexDirection": "column"}}>
<span style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "letterSpacing": "0.04em", "color": "#95a3b4"}}>03</span>
<svg className="plr-step-icon" aria-hidden="true" width="19" height="19" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" style={{"marginTop": "12px", "display": "block"}}><path d="M1.5 5.5A1.5 1.5 0 0 1 3 4h1.8l1.1-1.6h4.2L11.2 4H13a1.5 1.5 0 0 1 1.5 1.5V12A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12z"></path><circle cx="8" cy="8.6" r="2.3"></circle></svg>
<h3 style={{"margin": "8px 0 0", "fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#eef9fe"}}>Selfie liveness</h3>
<p style={{"margin": "5px 0 0", "fontSize": "12px", "fontWeight": "300", "lineHeight": "16px", "color": "#a6b4c0"}}>A simple blink check proves it’s really you behind the screen.</p>
</li>
<li className="plr-v-step plr-step" style={{"boxSizing": "border-box", "minHeight": "144px", "padding": "20px 16px 18px", "borderRadius": "12px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 10px 30px -14px rgba(6, 10, 26, 0.35)", "display": "flex", "flexDirection": "column"}}>
<span style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "letterSpacing": "0.04em", "color": "#95a3b4"}}>04</span>
<svg className="plr-step-icon" aria-hidden="true" width="19" height="19" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" style={{"marginTop": "12px", "display": "block"}}><path d="M13 5.5V3.75A1.75 1.75 0 0 0 11.25 2h-8A1.75 1.75 0 0 0 1.5 3.75v8.5A1.75 1.75 0 0 0 3.25 14h8A1.75 1.75 0 0 0 13 12.25V10.5"></path><rect x="9" y="5.5" width="5.5" height="5" rx="1.5"></rect><circle cx="11.6" cy="8" r="0.55" fill={v.accent} stroke="none"></circle></svg>
<h3 style={{"margin": "8px 0 0", "fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#eef9fe"}}>Link your wallet</h3>
<p style={{"margin": "5px 0 0", "fontSize": "12px", "fontWeight": "300", "lineHeight": "16px", "color": "#a6b4c0"}}>Connect your Varial wallet for payouts.</p>
</li>
<li className="plr-v-step plr-step" style={{"boxSizing": "border-box", "minHeight": "144px", "padding": "20px 16px 18px", "borderRadius": "12px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 10px 30px -14px rgba(6, 10, 26, 0.35)", "display": "flex", "flexDirection": "column"}}>
<span style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "letterSpacing": "0.04em", "color": "#95a3b4"}}>05</span>
<svg className="plr-step-icon" aria-hidden="true" width="19" height="19" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" style={{"marginTop": "12px", "display": "block"}}><path d="M1.5 3.5h10M1.5 7.5h8M1.5 11.5h5.5M9.5 11.5l1.6 1.6 3.4-3.4"></path></svg>
<h3 style={{"margin": "8px 0 0", "fontSize": "15px", "fontWeight": "500", "lineHeight": "20px", "color": "#eef9fe"}}>Review &amp; submit</h3>
<p style={{"margin": "5px 0 0", "fontSize": "12px", "fontWeight": "300", "lineHeight": "16px", "color": "#a6b4c0"}}>Confirm everything in one summary and you’re verified.</p>
</li>
</ol>
<div className="plr-vstage" aria-hidden="true">
<div className="plr-vstage-in">
<div className="plr-vhd"><span className="plr-vhd-l"><span className="plr-vhd-dot"></span>PrimeLendRow · Verification</span><span className="plr-vhd-r">Step {v.vStepTxt} / 05</span></div>
<div className="plr-vin">
<div className="plr-vtrail"><span><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>Live face</span></div>
<div className="plr-vdoc"><div className="plr-vdoc-in">
<div className="plr-vidw"><div className="plr-vidc">
<div className="plr-vidc-top"><span>Identity card</span><span className="plr-vidc-tag">Sample</span></div>
<div className="plr-vidc-body"><div className="plr-vidc-ph"><svg viewBox="0 0 86 110" width="86" height="110" aria-hidden="true"><circle cx="43" cy="44" r="19"></circle><path d="M8 110c3-24 17-36 35-36s32 12 35 36z"></path></svg></div>
<div className="plr-vidc-f"><span className="plr-vidc-no">1234-5678-9012-3456</span><span className="plr-vidc-kv"><span>Last name</span><b>DELA CRUZ</b></span><span className="plr-vidc-kv"><span>Given names</span><b>JUAN</b></span><span className="plr-vidc-kv"><span>Date of birth</span><b>01 JAN 1990</b></span></div></div>
<span className="plr-vscan2"></span>
</div><div className="plr-vframe"><i></i><i></i><i></i><i></i></div></div>
<div className="plr-vupl"><div className="plr-vupl-st"><span className="plr-vupl-a">Checking image quality</span><span className="plr-vupl-b"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>Image is clear</span></div><span className="plr-vqc"><span><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>Sharp</span><span><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>No glare</span><span><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>All corners</span></span></div>
</div></div>
<div className="plr-vpnl plr-vp1" style={{"--x": "60px", "--y": "50px", "width": "400px"} as CSSProperties}><div className="plr-vcard">
<span className="plr-vct">Scanned details</span>
<div className="plr-vfrow" style={{"--d": ".1s"} as CSSProperties}><span className="plr-vkv"><span>Full name</span><b>Juan Dela Cruz</b></span><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg></div>
<div className="plr-vfrow" style={{"--d": ".25s"} as CSSProperties}><span className="plr-vkv"><span>Date of birth</span><b>01 / 01 / 1990</b></span><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg></div>
<div className="plr-vfrow" style={{"--d": ".4s"} as CSSProperties}><span className="plr-vkv"><span>ID type</span><b>National ID</b></span><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg></div>
<div className="plr-vfrow" style={{"--d": ".55s"} as CSSProperties}><span className="plr-vkv"><span>ID number</span><b>•••• •••• 3456</b></span><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg></div>
</div></div>
<div className="plr-vpnl plr-vp2" style={{"--x": "96px", "--y": "16px", "width": "300px"} as CSSProperties}><div className="plr-vcard sm">
<div className="plr-vself-hd"><span>Live selfie</span><span className="plr-vfcheck"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg></span></div>
<div className="plr-vcam2"><svg viewBox="0 0 272 280" width="272" height="280" aria-hidden="true"><path className="b" d="M56 280c8-52 40-74 80-74s72 22 80 74z"></path><rect className="n" x="122" y="160" width="28" height="40" rx="10"></rect><ellipse className="f" cx="136" cy="118" rx="44" ry="54"></ellipse><path className="h" d="M92 106c0-34 19-52 44-52s44 18 44 52c-7-18-24-27-44-27s-37 9-44 27z"></path><ellipse className="g" cx="136" cy="120" rx="74" ry="92"></ellipse><path className="plr-vfring" d="M136 28a74 92 0 1 1 0 184a74 92 0 1 1 0-184" pathLength="100"></path><circle className="plr-vfpt" style={{"--d": ".5s"} as CSSProperties} cx="118" cy="114" r="3"></circle><circle className="plr-vfpt" style={{"--d": ".6s"} as CSSProperties} cx="154" cy="114" r="3"></circle><circle className="plr-vfpt" style={{"--d": ".7s"} as CSSProperties} cx="136" cy="132" r="3"></circle><circle className="plr-vfpt" style={{"--d": ".8s"} as CSSProperties} cx="122" cy="150" r="3"></circle><circle className="plr-vfpt" style={{"--d": ".9s"} as CSSProperties} cx="150" cy="150" r="3"></circle><circle className="plr-vfpt" style={{"--d": "1s"} as CSSProperties} cx="136" cy="88" r="3"></circle></svg>
<div className="plr-vpills"><span className="plr-vpa">Blink once</span><span className="plr-vpb"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#6fd6a5" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>Live check passed</span></div>
</div></div></div>
<div className="plr-vpnl plr-vp3" style={{"--x": "80px", "--y": "10px", "width": "380px"} as CSSProperties}><div className="plr-vcard">
<span className="plr-vct">Link your wallet</span>
<div className="plr-vwsel"><span className="plr-vwt pri">V</span><span className="plr-vwn">Varial</span><span className="plr-vwst"><span className="plr-vwa"><svg className="plr-vspin" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" aria-hidden="true"><path d="M21 12a9 9 0 1 1-6.219-8.56"></path></svg>Approve…</span><span className="plr-vwb"><i></i>Connected</span></span></div>
<div className="plr-vwsig"><svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M12 20h9"></path><path d="M16.4 3.6a2.1 2.1 0 0 1 3 3L7.4 18.6a2 2 0 0 1-.9.5l-2.9.8a.5.5 0 0 1-.6-.6l.8-2.9a2 2 0 0 1 .5-.9z"></path></svg><span className="t">Ownership by signature</span><span className="pill">No funds move</span></div>
</div></div>
<div className="plr-vpnl plr-vp4" style={{"--x": "0px", "--y": "0px", "width": "440px"} as CSSProperties}><div className="plr-vcard">
<span className="plr-vct">Review your details</span>
<div className="plr-vrrow" style={{"--d": ".05s"} as CSSProperties}><span className="plr-vic-id"></span><span className="t">Identity document</span><span className="plr-vrck"><svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg></span></div>
<div className="plr-vrrow" style={{"--d": ".2s"} as CSSProperties}><span className="plr-vic-face"></span><span className="t">Face check</span><span className="plr-vrck"><svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg></span></div>
<div className="plr-vrrow" style={{"--d": ".35s"} as CSSProperties}><span className="plr-vwt pri sm">V</span><span className="t">Wallet <span className="m">GDX4…7KP</span></span><span className="plr-vrck"><svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg></span></div>
<div className="plr-vrbtn"><span className="plr-vrb-a">Sign &amp; submit</span><span className="plr-vrb-b"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>Submitted</span></div>
<div className="plr-vrstat"><span className="s1"><i></i>Submitted</span><span className="ln"></span><span className="s2"><i></i>In review</span><span className="ln"></span><span className="s3"><i></i>Verified</span></div>
</div></div>
</div>
</div>
</div>
</div>
</div>
</div>
</div>
</section>
<section id="plr-lend" className={"plr-screen plr-split plr-sec " + v.sec.lend} aria-labelledby="plr-lend-title" style={{"position": "relative", "display": "grid", "gridTemplateColumns": "repeat(2, minmax(0, 1fr))", "alignItems": "center", "columnGap": "48px", "rowGap": "48px", "boxSizing": "border-box", "padding": "82px clamp(20px, 4.95vw, 95px) 84px"}}>
<div style={{"position": "relative", "minWidth": "0", "display": "flex", "flexDirection": "column", "alignItems": "flex-start"}}>
<h2 id="plr-lend-title" className="plr-s-h2" style={{"margin": "0", "maxWidth": "620px", "fontSize": "56px", "fontWeight": "400", "lineHeight": "1.08", "letterSpacing": "0", "color": "#eef9fe"}}>Put idle capital <span style={{"color": "var(--landing-accent)"}}>to work.</span></h2>
<p className="plr-s-sub" style={{"margin": "14px 0 0", "maxWidth": "420px", "fontSize": "15px", "fontWeight": "400", "lineHeight": "20px", "color": "#bbc6d1"}}>Deposit with PayPal or Stripe. Money that isn’t being used can be withdrawn anytime.</p>
<Link className="plr-s-cta plr-btn" style={{"marginTop": "26px", "display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "33px", "padding": "0 22px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "12px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)"}} to="/lending">Start Lending</Link>
</div>
<div className="plr-card-wrap" style={{"position": "relative", "width": "416px", "maxWidth": "100%", "justifySelf": "center"}}>
<div className="plr-panel" style={{"position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "28px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backgroundClip": "padding-box", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "18px", "color": "#eef9fe"}}>
<div className="plr-s-item" style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "animationDelay": "400ms"}}>
<span style={{"fontSize": "12px", "fontWeight": "600", "lineHeight": "16px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>How lending works</span>
<span style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c5cfdb", "padding": "4px 10px", "borderRadius": "999px", "border": "1px solid rgba(190, 207, 245, 0.2)", "background": "rgba(163, 184, 241, 0.06)"}}>Pooled deposits</span>
</div>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "470ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><path d="M8 2l6 3-6 3-6-3z"></path><path d="M2 8.25l6 3 6-3"></path><path d="M2 11.25l6 3 6-3"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Deposit</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Add money to the pool with PayPal or Stripe.</span></div>
</li>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "540ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="7" width="10" height="7" rx="1.5"></rect><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Locked while in use</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Money that’s loaned out, backing a loan, or pledged stays put.</span></div>
</li>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "610ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><path d="M8 10V2.5"></path><path d="M5 5.5l3-3 3 3"></path><path d="M2.5 9.5v2.5a1.5 1.5 0 0 0 1.5 1.5h8a1.5 1.5 0 0 0 1.5-1.5V9.5"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Withdraw the rest</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Anything not in use is yours to withdraw.</span></div>
</li>
</ul>
<p className="plr-s-item" style={{"margin": "0", "paddingTop": "16px", "borderTop": "1px solid rgba(190, 207, 245, 0.14)", "display": "flex", "alignItems": "center", "gap": "8px", "fontSize": "12px", "lineHeight": "16px", "color": "#c5cfdb", "animationDelay": "680ms"}}><svg aria-hidden="true" width="14" height="14" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 14px"}}><path d="M3 2.5h8.5A1.5 1.5 0 0 1 13 4v9.5H4.5A1.5 1.5 0 0 1 3 12z"></path><path d="M5.5 5.5h5M5.5 8h5"></path></svg>Every movement is recorded with double-entry accounting.</p>
</div>
</div>
</section>
<section id="plr-borrow" className={"plr-screen plr-split plr-sec " + v.sec.borrow} aria-labelledby="plr-borrow-title" style={{"position": "relative", "display": "grid", "gridTemplateColumns": "repeat(2, minmax(0, 1fr))", "alignItems": "center", "columnGap": "48px", "rowGap": "48px", "boxSizing": "border-box", "padding": "82px clamp(20px, 4.95vw, 95px) 84px"}}>
<div style={{"position": "relative", "minWidth": "0", "display": "flex", "flexDirection": "column", "alignItems": "flex-start"}}>
<h2 id="plr-borrow-title" className="plr-s-h2" style={{"margin": "0", "maxWidth": "620px", "fontSize": "56px", "fontWeight": "400", "lineHeight": "1.08", "letterSpacing": "0", "color": "#eef9fe"}}>Borrow against <span style={{"color": "var(--landing-accent)"}}>what you have.</span></h2>
<p className="plr-s-sub" style={{"margin": "14px 0 0", "maxWidth": "420px", "fontSize": "15px", "fontWeight": "400", "lineHeight": "20px", "color": "#bbc6d1"}}>Borrow against your deposit, with XLM collateral, or with a guarantor’s backing.</p>
<Link className="plr-s-cta plr-btn" style={{"marginTop": "26px", "display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "33px", "padding": "0 22px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "12px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)"}} to="/borrow">Start Borrowing</Link>
</div>
<div className="plr-card-wrap" style={{"position": "relative", "width": "416px", "maxWidth": "100%", "justifySelf": "center"}}>
<div className="plr-panel" style={{"position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "28px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backgroundClip": "padding-box", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "18px", "color": "#eef9fe"}}>
<div className="plr-s-item" style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "animationDelay": "400ms"}}>
<span style={{"fontSize": "12px", "fontWeight": "600", "lineHeight": "16px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>How borrowing works</span>
</div>
<ol style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "470ms"}}>
<span className="plr-row-icon" aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": "12px", "fontWeight": "700", "lineHeight": "16px", "color": "var(--landing-accent)"}}>01</span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Choose your backing</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Your deposit, XLM worth 120% of the loan, or a guarantor.</span></div>
</li>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "540ms"}}>
<span className="plr-row-icon" aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": "12px", "fontWeight": "700", "lineHeight": "16px", "color": "var(--landing-accent)"}}>02</span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>See your limit</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Your tier sets your limit. A guarantor doubles it.</span></div>
</li>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "610ms"}}>
<span className="plr-row-icon" aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": "12px", "fontWeight": "700", "lineHeight": "16px", "color": "var(--landing-accent)"}}>03</span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Get your loan</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Borrow any amount up to your limit.</span></div>
</li>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "680ms"}}>
<span className="plr-row-icon" aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": "12px", "fontWeight": "700", "lineHeight": "16px", "color": "var(--landing-accent)"}}>04</span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Repay and grow your score</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Your 50–150 score updates at the end of each term.</span></div>
</li>
</ol>
</div>
</div>
</section>
<section id="plr-pay" className={"plr-screen plr-split plr-sec " + v.sec.pay} aria-labelledby="plr-pay-title" style={{"position": "relative", "display": "grid", "gridTemplateColumns": "repeat(2, minmax(0, 1fr))", "alignItems": "center", "columnGap": "48px", "rowGap": "48px", "boxSizing": "border-box", "padding": "82px clamp(20px, 4.95vw, 95px) 84px"}}>
<div style={{"position": "relative", "minWidth": "0", "display": "flex", "flexDirection": "column", "alignItems": "flex-start"}}>
<h2 id="plr-pay-title" className="plr-s-h2" style={{"margin": "0", "maxWidth": "620px", "fontSize": "56px", "fontWeight": "400", "lineHeight": "1.08", "letterSpacing": "0", "color": "#eef9fe"}}>Repayments, <span style={{"color": "var(--landing-accent)"}}>kept simple.</span></h2>
<p className="plr-s-sub" style={{"margin": "14px 0 0", "maxWidth": "420px", "fontSize": "15px", "fontWeight": "400", "lineHeight": "20px", "color": "#bbc6d1"}}>Pay with PayPal or Stripe. Interest is charged only on what you still owe.</p>
<Link className="plr-s-cta plr-btn" style={{"marginTop": "26px", "display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "33px", "padding": "0 22px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "12px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)"}} to="/records">View Records</Link>
</div>
<div className="plr-card-wrap" style={{"position": "relative", "width": "416px", "maxWidth": "100%", "justifySelf": "center"}}>
<div className="plr-panel" style={{"position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "28px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backgroundClip": "padding-box", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "18px", "color": "#eef9fe"}}>
<div className="plr-s-item" style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "animationDelay": "400ms"}}>
<span style={{"fontSize": "12px", "fontWeight": "600", "lineHeight": "16px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>Repayments</span>
<span style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c5cfdb", "padding": "4px 10px", "borderRadius": "999px", "border": "1px solid rgba(190, 207, 245, 0.2)", "background": "rgba(163, 184, 241, 0.06)"}}>PayPal · Stripe</span>
</div>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "470ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><rect x="1.5" y="3.5" width="13" height="9" rx="1.75"></rect><path d="M1.5 6.5h13M4 10h2.5"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Pay with PayPal or Stripe</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>No wallet to connect.</span></div>
</li>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "540ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><ellipse cx="8" cy="4.5" rx="5" ry="2"></ellipse><path d="M3 4.5V8c0 1.1 2.2 2 5 2s5-.9 5-2V4.5"></path><path d="M3 8v3.5c0 1.1 2.2 2 5 2s5-.9 5-2V8"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Deposit or XLM-backed loans</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>2% to 1% interest per month.</span></div>
</li>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "610ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><circle cx="6" cy="5" r="2.5"></circle><path d="M1.5 13.5c0-2.5 2-4 4.5-4s4.5 1.5 4.5 4"></path><path d="M10.5 7.5l1.5 1.5 3-3"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Guarantor-backed loans</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>6% to 3% interest per month.</span></div>
</li>
<li className="plr-s-item plr-row" style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "backgroundClip": "padding-box", "border": "1px solid rgba(190, 207, 245, 0.14)", "animationDelay": "680ms"}}>
<span className="plr-row-icon" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "color": "var(--landing-accent)"}}><svg aria-hidden="true" width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"><path d="M2 4l4 4 3-3 5 6"></path><path d="M10.5 11H14V7.5"></path></svg></span>
<div style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Interest shrinks as you pay</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>It’s charged on your remaining principal.</span></div>
</li>
</ul>
</div>
</div>
</section>
<section id="plr-interest" className={"plr-screen plr-split plr-sec " + v.sec.interest} aria-labelledby="plr-interest-title" style={{"position": "relative", "display": "grid", "gridTemplateColumns": "repeat(2, minmax(0, 1fr))", "alignItems": "center", "columnGap": "48px", "rowGap": "48px", "boxSizing": "border-box", "padding": "82px clamp(20px, 4.95vw, 95px) 84px"}}>
<div style={{"position": "relative", "minWidth": "0", "display": "flex", "flexDirection": "column", "alignItems": "flex-start"}}>
<h2 id="plr-interest-title" className="plr-s-h2" style={{"margin": "0", "maxWidth": "620px", "fontSize": "56px", "fontWeight": "400", "lineHeight": "1.08", "letterSpacing": "0", "color": "#eef9fe"}}>Where interest <span style={{"color": "var(--landing-accent)"}}>goes.</span></h2>
<p className="plr-s-sub" style={{"margin": "14px 0 0", "maxWidth": "420px", "fontSize": "15px", "fontWeight": "400", "lineHeight": "20px", "color": "#bbc6d1"}}>Each interest payment is split the same way. Depositors earn mostly from XLM-backed loans, on the part of their money that was lent.</p>
<a href="https://kentice.gitbook.io/lendrow" className="plr-s-cta plr-btn" style={{"marginTop": "26px", "display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "33px", "padding": "0 22px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "12px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)"}} target="_blank" rel="noreferrer">Read Docs</a>
</div>
<div className="plr-card-wrap" style={{"position": "relative", "width": "416px", "maxWidth": "100%", "justifySelf": "center"}}>
<div className="plr-panel" style={{"position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "28px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backgroundClip": "padding-box", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "gap": "18px", "color": "#eef9fe"}}>
<div className="plr-s-item" style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "animationDelay": "400ms"}}>
<span style={{"fontSize": "12px", "fontWeight": "600", "lineHeight": "16px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>Interest split</span>
<span style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c5cfdb", "padding": "4px 10px", "borderRadius": "999px", "border": "1px solid rgba(190, 207, 245, 0.2)", "background": "rgba(163, 184, 241, 0.06)"}}>Every payment</span>
</div>
<div className="plr-s-item" aria-hidden="true" style={{"display": "flex", "gap": "3px", "height": "8px", "animationDelay": "450ms"}}><span style={{"flex": "40 1 0", "borderRadius": "999px", "background": "#5b8def"}}></span><span style={{"flex": "20 1 0", "borderRadius": "999px", "background": "#e27a3f"}}></span><span style={{"flex": "10 1 0", "borderRadius": "999px", "background": "#43b07a"}}></span><span style={{"flex": "30 1 0", "borderRadius": "999px", "background": "#e05a87"}}></span></div>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column"}}>
<li className="plr-s-item" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "animationDelay": "500ms"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#5b8def"}}></span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Depositors</span><span style={{"fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Mostly from XLM-backed loans, shared by how much of each one’s money was lent.</span></span>
<span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>40%</span>
</li>
<li className="plr-s-item" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "borderTop": "1px solid rgba(190, 207, 245, 0.1)", "animationDelay": "560ms"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#e27a3f"}}></span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Lending reserve</span></span>
<span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>20%</span>
</li>
<li className="plr-s-item" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "borderTop": "1px solid rgba(190, 207, 245, 0.1)", "animationDelay": "620ms"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#43b07a"}}></span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Platform</span></span>
<span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>10%</span>
</li>
<li className="plr-s-item" style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "borderTop": "1px solid rgba(190, 207, 245, 0.1)", "animationDelay": "680ms"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#e05a87"}}></span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}><span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>Recovery fund</span></span>
<span style={{"fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "color": "#eef9fe"}}>30%</span>
</li>
</ul>
<p className="plr-s-item" style={{"margin": "0", "paddingTop": "14px", "borderTop": "1px solid rgba(190, 207, 245, 0.14)", "display": "flex", "alignItems": "flex-start", "gap": "8px", "fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb", "animationDelay": "760ms"}}><svg aria-hidden="true" width="14" height="14" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 14px", "marginTop": "2px"}}><path d="M6 7.2a2.4 2.4 0 1 0 0-4.8 2.4 2.4 0 0 0 0 4.8z"></path><path d="M1.5 13.5c0-2.4 2-4 4.5-4s4.5 1.6 4.5 4"></path><path d="M11 3.2a2.2 2.2 0 0 1 0 4.2M12.5 9.7c1.2.5 2 1.6 2 3.3"></path></svg>With guarantors (up to 2), they share 10–25% pro rata, taken from the recovery fund.</p>
</div>
</div>
</section>
<section id="plr-faq" className={"plr-screen plr-split plr-sec " + v.sec.faq} aria-labelledby="plr-faq-title" style={{"position": "relative", "display": "grid", "gridTemplateColumns": "repeat(2, minmax(0, 1fr))", "alignItems": "center", "columnGap": "48px", "rowGap": "48px", "boxSizing": "border-box", "padding": "82px clamp(20px, 4.95vw, 95px) 84px"}}>
<div style={{"position": "relative", "minWidth": "0", "display": "flex", "flexDirection": "column", "alignItems": "flex-start"}}>
<h2 id="plr-faq-title" className="plr-s-h2" style={{"margin": "0", "maxWidth": "620px", "fontSize": "56px", "fontWeight": "400", "lineHeight": "1.08", "letterSpacing": "0", "color": "#eef9fe"}}>Questions, <span style={{"color": "var(--landing-accent)"}}>answered.</span></h2>
<p className="plr-s-sub" style={{"margin": "14px 0 0", "maxWidth": "420px", "fontSize": "15px", "fontWeight": "400", "lineHeight": "20px", "color": "#bbc6d1"}}>Short answers to what members ask most. The docs cover the details.</p>
<a href="https://kentice.gitbook.io/lendrow" className="plr-s-cta plr-btn" style={{"marginTop": "26px", "display": "inline-flex", "alignItems": "center", "justifyContent": "center", "height": "33px", "padding": "0 22px", "boxSizing": "border-box", "borderRadius": "999px", "background": "linear-gradient(180deg, #3f6ee9 0%, #2f54d4 100%)", "color": "#ffffff", "fontSize": "12px", "fontWeight": "700", "textDecoration": "none", "boxShadow": "0 6px 18px rgba(50, 90, 230, 0.45)"}} target="_blank" rel="noreferrer">Read Docs</a>
</div>
<div className="plr-card-wrap" style={{"position": "relative", "width": "416px", "maxWidth": "100%", "justifySelf": "center"}}>
<div className="plr-panel" style={{"position": "relative", "boxSizing": "border-box", "width": "100%", "padding": "24px 22px", "borderRadius": "18px", "border": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backgroundClip": "padding-box", "backdropFilter": "var(--landing-glass-card-filter)", "WebkitBackdropFilter": "var(--landing-glass-card-filter)", "boxShadow": "0 1px 2px rgba(5, 9, 24, 0.25), 0 12px 24px -10px rgba(6, 10, 26, 0.4), 0 32px 64px -28px rgba(6, 10, 26, 0.55), inset 0 1px 0 rgba(255, 255, 255, 0.09)", "display": "flex", "flexDirection": "column", "color": "#eef9fe"}}>
<div className="plr-s-item" style={{"padding": "0 6px 12px", "animationDelay": "400ms"}}><span style={{"fontSize": "12px", "fontWeight": "600", "lineHeight": "16px", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "var(--landing-accent)"}}>Frequently asked</span></div>
<div className="plr-s-item" style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)", "animationDelay": "470ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-faq-q1" className="plr-faq-q" aria-expanded={v.faq.o1} aria-controls="plr-faq-a1" onClick={v.onFaq1} style={{"width": "100%", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 6px", "border": "0", "borderRadius": "10px", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "textAlign": "left", "cursor": "pointer"}}>Who can join PrimeLendRow?<svg aria-hidden="true" width="14" height="14" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 14px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-faq-a1" className={"plr-faq-a " + v.faq.c1} role="region" aria-labelledby="plr-faq-q1"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 6px 14px", "fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Membership is KYC-gated. Complete the five verification steps once to borrow or lend.</p></div></div>
</div>
<div className="plr-s-item" style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)", "animationDelay": "540ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-faq-q2" className="plr-faq-q" aria-expanded={v.faq.o2} aria-controls="plr-faq-a2" onClick={v.onFaq2} style={{"width": "100%", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 6px", "border": "0", "borderRadius": "10px", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "textAlign": "left", "cursor": "pointer"}}>What can I borrow against?<svg aria-hidden="true" width="14" height="14" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 14px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-faq-a2" className={"plr-faq-a " + v.faq.c2} role="region" aria-labelledby="plr-faq-q2"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 6px 14px", "fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Your deposit, XLM worth 120% of the loan, or a guarantor, who doubles your limit at every tier.</p></div></div>
</div>
<div className="plr-s-item" style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)", "animationDelay": "610ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-faq-q3" className="plr-faq-q" aria-expanded={v.faq.o3} aria-controls="plr-faq-a3" onClick={v.onFaq3} style={{"width": "100%", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 6px", "border": "0", "borderRadius": "10px", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "textAlign": "left", "cursor": "pointer"}}>How does my credit score change?<svg aria-hidden="true" width="14" height="14" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 14px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-faq-a3" className={"plr-faq-a " + v.faq.c3} role="region" aria-labelledby="plr-faq-q3"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 6px 14px", "fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Scores run from 50 to 150 and only update at the end of each loan term.</p></div></div>
</div>
<div className="plr-s-item" style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)", "animationDelay": "680ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-faq-q4" className="plr-faq-q" aria-expanded={v.faq.o4} aria-controls="plr-faq-a4" onClick={v.onFaq4} style={{"width": "100%", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 6px", "border": "0", "borderRadius": "10px", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "textAlign": "left", "cursor": "pointer"}}>How do repayments work?<svg aria-hidden="true" width="14" height="14" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 14px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-faq-a4" className={"plr-faq-a " + v.faq.c4} role="region" aria-labelledby="plr-faq-q4"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 6px 14px", "fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Pay with PayPal or Stripe. Interest is charged on your remaining principal, so it drops as you pay.</p></div></div>
</div>
<div className="plr-s-item" style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)", "animationDelay": "750ms"}}>
<h3 style={{"margin": "0"}}><button type="button" id="plr-faq-q5" className="plr-faq-q" aria-expanded={v.faq.o5} aria-controls="plr-faq-a5" onClick={v.onFaq5} style={{"width": "100%", "display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "12px", "padding": "14px 6px", "border": "0", "borderRadius": "10px", "background": "transparent", "color": "#eef9fe", "fontFamily": "inherit", "fontSize": "15px", "fontWeight": "600", "lineHeight": "20px", "textAlign": "left", "cursor": "pointer"}}>How do lenders earn?<svg aria-hidden="true" width="14" height="14" viewBox="0 0 16 16" fill="none" stroke={v.accent} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{"flex": "0 0 14px"}}><path d="M4 6l4 4 4-4"></path></svg></button></h3>
<div id="plr-faq-a5" className={"plr-faq-a " + v.faq.c5} role="region" aria-labelledby="plr-faq-q5"><div className="plr-faq-a-in"><p style={{"margin": "0", "padding": "0 6px 14px", "fontSize": "12px", "lineHeight": "18px", "color": "#c5cfdb"}}>Depositors get 40% of interest, mostly from XLM-backed loans, shared by how much of each one’s money was lent.</p></div></div>
</div>
</div>
</div>
</section>
</main>
<footer className={"plr-footer " + v.footerState} style={{"position": "relative", "zIndex": "1", "minHeight": "58px", "boxSizing": "border-box", "padding": "0 24px 0 26.5px", "display": "flex", "flexWrap": "wrap", "alignItems": "center", "justifyContent": "space-between", "columnGap": "16px", "borderTop": "1px solid var(--landing-glass-border)", "background": "var(--landing-glass-bg)", "backdropFilter": "var(--landing-glass-chrome-filter)", "WebkitBackdropFilter": "var(--landing-glass-chrome-filter)"}}>
<p style={{"margin": "0", "fontSize": "12px", "fontWeight": "400", "lineHeight": "16px", "color": "#95a3b4"}}>© 2026 PrimeLendRow. All rights reserved.</p>
<nav aria-label="Footer" style={{"display": "flex", "alignItems": "center", "gap": "20px"}}>
<a href="https://kentice.gitbook.io/lendrow" className="plr-link" style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c2cdd7", "textDecoration": "none"}} target="_blank" rel="noreferrer">Docs</a>
<a href="#terms" className="plr-link" style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c2cdd7", "textDecoration": "none"}} onClick={v.onTerms} aria-haspopup="dialog">Terms</a>
<a href="#privacy" className="plr-link" style={{"fontSize": "12px", "fontWeight": "500", "lineHeight": "16px", "color": "#c2cdd7", "textDecoration": "none"}} onClick={v.onPrivacy} aria-haspopup="dialog">Privacy</a>
</nav>
</footer>
</div>
    )
}
