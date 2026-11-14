import type { LandingValues } from './useLandingPresentation'

type Props = { v: LandingValues; mobile: boolean }

export default function LandingDocuments({ v, mobile }: Props) {
    return (<>
{v.dlgOpen && (<>
<div className={"plr-dlg-overlay " + v.dlgCls} onClick={v.onDlgOverlay}>
<div className="plr-dlg" role="dialog" aria-modal="true" aria-labelledby="plr-dlg-title" onKeyDown={v.onDlgKey}>
<div className="plr-dlg-head">
<div className="plr-dlg-tabs" role="tablist" aria-label="Documents">
<button type="button" role="tab" id="plr-tab-docs" className={"plr-dlg-tab " + v.dlgTabCls.docs} aria-selected={v.dlgSel.docs} aria-controls="plr-dlg-body" tabIndex={v.dlgTi.docs} onClick={v.onTabDocs}>Docs</button>
<button type="button" role="tab" id="plr-tab-terms" className={"plr-dlg-tab " + v.dlgTabCls.terms} aria-selected={v.dlgSel.terms} aria-controls="plr-dlg-body" tabIndex={v.dlgTi.terms} onClick={v.onTabTerms}>Terms</button>
<button type="button" role="tab" id="plr-tab-privacy" className={"plr-dlg-tab " + v.dlgTabCls.privacy} aria-selected={v.dlgSel.privacy} aria-controls="plr-dlg-body" tabIndex={v.dlgTi.privacy} onClick={v.onTabPrivacy}>Privacy</button>
</div>
<button id="plr-dlg-close" type="button" className="plr-dlg-x" onClick={v.closeDlg} aria-label="Close">
<svg aria-hidden="true" width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
<path d="M4 4l8 8M12 4l-8 8">

</path>
</svg>
</button>
</div>
<div id="plr-dlg-body" className={"plr-dlg-body " + v.dlgBodyCls} role="tabpanel" tabIndex={0} onScroll={v.onDlgScroll}>
{v.dlgIs.docs && (<>
<div className="plr-dlg-pane">
<div>
<h2 id="plr-dlg-title" className="plr-dlg-title">How PrimeLendRow <span style={{"color": "var(--landing-accent)"}}>works.</span></h2>
<p className="plr-dlg-sub">The short version of lending, borrowing, repayments and interest on PrimeLendRow.</p>
</div>
<section id="dlg-doc-verify" className="plr-dlg-sec" aria-labelledby="dlg-doc-verify-h">
<h3 id="dlg-doc-verify-h">Get verified</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Everyone verifies once before lending or borrowing. It takes five steps.</p>
<ol style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": (mobile ? "15px" : "12px"), "fontWeight": "700", "color": "var(--landing-accent)"}}>01</span>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Upload your ID</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>OCR reads your details from a government ID.</span>
</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": (mobile ? "15px" : "12px"), "fontWeight": "700", "color": "var(--landing-accent)"}}>02</span>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Live document scan</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>A quick scan checks the document in real time.</span>
</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": (mobile ? "15px" : "12px"), "fontWeight": "700", "color": "var(--landing-accent)"}}>03</span>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Selfie liveness</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>A blink check confirms it’s really you.</span>
</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": (mobile ? "15px" : "12px"), "fontWeight": "700", "color": "var(--landing-accent)"}}>04</span>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Link your wallet</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>Connect your Varial wallet for payouts.</span>
</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "gap": "14px", "padding": "12px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span aria-hidden="true" style={{"flex": "0 0 36px", "height": "36px", "display": "grid", "placeItems": "center", "borderRadius": "11px", "background": "rgba(163, 184, 241, 0.16)", "fontSize": (mobile ? "15px" : "12px"), "fontWeight": "700", "color": "var(--landing-accent)"}}>05</span>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Review &amp; submit</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>Confirm everything in one summary.</span>
</span>
</li>
</ol>
</section>
<section id="dlg-doc-lend" className="plr-dlg-sec" aria-labelledby="dlg-doc-lend-h">
<h3 id="dlg-doc-lend-h">Lending</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Deposit into the pool with PayPal or Stripe.</p>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "14px", "padding": "12px 14px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Locked while in use</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>Money that’s loaned out, backing a loan, or pledged as a guarantee stays put.</span>
</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "14px", "padding": "12px 14px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Free to withdraw</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>Anything not in use can be withdrawn anytime.</span>
</span>
</li>
</ul>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Depositors earn a share of interest, mostly from XLM-backed loans.</p>
</section>
<section id="dlg-doc-borrow" className="plr-dlg-sec" aria-labelledby="dlg-doc-borrow-h">
<h3 id="dlg-doc-borrow-h">Borrowing</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Choose how to back your loan. Your tier sets your limit.</p>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "14px", "padding": "12px 14px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Against your deposit</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>Use your pool deposit as backing.</span>
</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "14px", "padding": "12px 14px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>XLM collateral</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>Lock XLM worth 120% of the loan.</span>
</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "14px", "padding": "12px 14px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Guarantor-backed</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>Up to 2 guarantors. Doubles your limit at every tier.</span>
</span>
</li>
</ul>
</section>
<section id="dlg-doc-repay" className="plr-dlg-sec" aria-labelledby="dlg-doc-repay-h">
<h3 id="dlg-doc-repay-h">Repayments</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Pay with PayPal or Stripe. Interest is charged on the principal you still owe, so it drops as you pay.</p>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px"}}>
<li style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "14px", "padding": "12px 14px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Deposit or XLM-backed</span>
</span>
<span style={{"flex": "0 0 auto", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "var(--landing-accent)"}}>2% to 1% / month</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "justifyContent": "space-between", "gap": "14px", "padding": "12px 14px", "borderRadius": "12px", "background": "rgba(120, 145, 215, 0.09)", "border": "1px solid rgba(190, 207, 245, 0.14)"}}>
<span style={{"display": "flex", "flexDirection": "column", "gap": "2px", "minWidth": "0"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Guarantor-backed</span>
</span>
<span style={{"flex": "0 0 auto", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "var(--landing-accent)"}}>6% to 3% / month</span>
</li>
</ul>
<figure style={{"margin": "0", "padding": "14px 16px 6px", "borderRadius": "12px", "background": "rgba(8, 15, 30, 0.25)", "border": "1px solid rgba(190, 207, 245, 0.12)"}}>
<figcaption style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#bbc6d1", "marginBottom": "10px"}}>Example: borrow ₱1,000 for 3 months at 2% per month.</figcaption>
<table style={{"width": "100%", "borderCollapse": "collapse"}}>
<thead>
<tr>
<th scope="col" style={{"padding": "0 0 8px", "textAlign": "left", "fontSize": "12px", "fontWeight": "600", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "#a6b4c0"}}>Month</th>
<th scope="col" style={{"padding": "0 0 8px", "textAlign": "right", "fontSize": "12px", "fontWeight": "600", "letterSpacing": "0.08em", "textTransform": "uppercase", "color": "#a6b4c0"}}>You pay</th>
</tr>
</thead>
<tbody>
<tr style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)"}}>
<th scope="row" style={{"padding": "10px 12px 10px 0", "textAlign": "left", "fontWeight": "400"}}>
<span style={{"display": "block", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Month 1</span>
<span style={{"display": "block", "fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#a6b4c0"}}><strong style={{"fontWeight": "600", "color": "#eef9fe"}}>₱333.33 principal</strong> + <strong style={{"fontWeight": "600", "color": "var(--landing-accent)"}}>₱20.00 interest</strong> (2% of ₱1,000 still owed)</span>
</th>
<td style={{"padding": "10px 0", "textAlign": "right", "verticalAlign": "top", "whiteSpace": "nowrap", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>₱353.33</td>
</tr>
<tr style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)"}}>
<th scope="row" style={{"padding": "10px 12px 10px 0", "textAlign": "left", "fontWeight": "400"}}>
<span style={{"display": "block", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Month 2</span>
<span style={{"display": "block", "fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#a6b4c0"}}><strong style={{"fontWeight": "600", "color": "#eef9fe"}}>₱333.33 principal</strong> + <strong style={{"fontWeight": "600", "color": "var(--landing-accent)"}}>₱13.33 interest</strong> (2% of ₱666.67 still owed)</span>
</th>
<td style={{"padding": "10px 0", "textAlign": "right", "verticalAlign": "top", "whiteSpace": "nowrap", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>₱346.66</td>
</tr>
<tr style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)"}}>
<th scope="row" style={{"padding": "10px 12px 10px 0", "textAlign": "left", "fontWeight": "400"}}>
<span style={{"display": "block", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Month 3</span>
<span style={{"display": "block", "fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#a6b4c0"}}><strong style={{"fontWeight": "600", "color": "#eef9fe"}}>₱333.34 principal</strong> + <strong style={{"fontWeight": "600", "color": "var(--landing-accent)"}}>₱6.67 interest</strong> (2% of ₱333.34 still owed)</span>
</th>
<td style={{"padding": "10px 0", "textAlign": "right", "verticalAlign": "top", "whiteSpace": "nowrap", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>₱340.01</td>
</tr>
<tr style={{"borderTop": "1px solid rgba(190, 207, 245, 0.12)"}}>
<th scope="row" style={{"padding": "10px 12px 10px 0", "textAlign": "left", "fontWeight": "400"}}>
<span style={{"display": "block", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Total</span>
<span style={{"display": "block", "fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#a6b4c0"}}><strong style={{"fontWeight": "600", "color": "#eef9fe"}}>₱1,000 principal</strong> + <strong style={{"fontWeight": "600", "color": "var(--landing-accent)"}}>₱40 interest</strong></span>
</th>
<td style={{"padding": "10px 0", "textAlign": "right", "verticalAlign": "top", "whiteSpace": "nowrap", "fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "var(--landing-accent)"}}>₱1,040.00</td>
</tr>
</tbody>
</table>
</figure>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>The PayPal or Stripe processing fee is added on top of each payment.</p>
</section>
<section id="dlg-doc-interest" className="plr-dlg-sec" aria-labelledby="dlg-doc-interest-h">
<h3 id="dlg-doc-interest-h">Where interest goes</h3>
<div aria-hidden="true" style={{"display": "flex", "gap": "3px", "height": "8px"}}>
<span style={{"flex": "40 1 0", "borderRadius": "999px", "background": "#5b8def"}}>

</span>
<span style={{"flex": "20 1 0", "borderRadius": "999px", "background": "#e27a3f"}}>

</span>
<span style={{"flex": "10 1 0", "borderRadius": "999px", "background": "#43b07a"}}>

</span>
<span style={{"flex": "30 1 0", "borderRadius": "999px", "background": "#e05a87"}}>

</span>
</div>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column"}}>
<li style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#5b8def"}}>

</span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Depositors</span>
<span style={{"fontSize": (mobile ? "15px" : "12px"), "lineHeight": (mobile ? "22px" : "18px"), "color": "#c5cfdb"}}>Pro rata to how much of each one’s money was lent.</span>
</span>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>40%</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "borderTop": "1px solid rgba(190, 207, 245, 0.12)"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#e27a3f"}}>

</span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Lending reserve</span>
</span>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>20%</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "borderTop": "1px solid rgba(190, 207, 245, 0.12)"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#43b07a"}}>

</span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Platform</span>
</span>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>10%</span>
</li>
<li style={{"display": "flex", "alignItems": "center", "gap": "12px", "padding": "11px 0", "borderTop": "1px solid rgba(190, 207, 245, 0.12)"}}>
<span aria-hidden="true" style={{"flex": "0 0 10px", "width": "10px", "height": "10px", "borderRadius": "50%", "background": "#e05a87"}}>

</span>
<span style={{"flex": "1", "minWidth": "0", "display": "flex", "flexDirection": "column", "gap": "2px"}}>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>Recovery fund</span>
</span>
<span style={{"fontSize": (mobile ? "18px" : "15px"), "fontWeight": "600", "lineHeight": (mobile ? "24px" : "20px"), "color": "#eef9fe"}}>30%</span>
</li>
</ul>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>With guarantors (up to 2), they share 10–25% pro rata, taken from the recovery fund.</p>
</section>
<section id="dlg-doc-score" className="plr-dlg-sec" aria-labelledby="dlg-doc-score-h">
<h3 id="dlg-doc-score-h">Credit score</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Your score runs from 50 to 150 and updates at the end of each loan term.</p>
</section>
<section id="dlg-doc-records" className="plr-dlg-sec" aria-labelledby="dlg-doc-records-h">
<h3 id="dlg-doc-records-h">Records</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Every deposit, loan and repayment is recorded with double-entry accounting and shows in your loan records.</p>
</section>
</div>
</>)}
{v.dlgIs.terms && (<>
<div className="plr-dlg-pane">
<div>
<h2 id="plr-dlg-title" className="plr-dlg-title">Terms of <span style={{"color": "var(--landing-accent)"}}>Service.</span></h2>
<p className="plr-dlg-sub">Plain-language terms for using PrimeLendRow.</p>
</div>
<section id="dlg-t-who" className="plr-dlg-sec" aria-labelledby="dlg-t-who-h">
<h3 id="dlg-t-who-h">1. About PrimeLendRow</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>PrimeLendRow lets verified members lend into a shared pool and borrow against what they have.</p>
</section>
<section id="dlg-t-accept" className="plr-dlg-sec" aria-labelledby="dlg-t-accept-h">
<h3 id="dlg-t-accept-h">2. Accepting these terms</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>By creating an account you agree to these terms. If you don’t agree, don’t use the service.</p>
</section>
<section id="dlg-t-elig" className="plr-dlg-sec" aria-labelledby="dlg-t-elig-h">
<h3 id="dlg-t-elig-h">3. Eligibility and verification</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>You must be at least 18 and complete identity verification before lending or borrowing. The information you give must be accurate and kept up to date.</p>
</section>
<section id="dlg-t-account" className="plr-dlg-sec" aria-labelledby="dlg-t-account-h">
<h3 id="dlg-t-account-h">4. Your account</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Keep your login secure. You’re responsible for activity on your account. Contact support right away if you think it’s been accessed without your permission.</p>
</section>
<section id="dlg-t-deposit" className="plr-dlg-sec" aria-labelledby="dlg-t-deposit-h">
<h3 id="dlg-t-deposit-h">5. Deposits and withdrawals</h3>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Deposits are made with PayPal or Stripe.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Money that’s loaned out, backing a loan, or pledged as a guarantee is locked until it’s released.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Money that isn’t in use can be withdrawn.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>PayPal or Stripe processing fees may apply.</li>
</ul>
</section>
<section id="dlg-t-loans" className="plr-dlg-sec" aria-labelledby="dlg-t-loans-h">
<h3 id="dlg-t-loans-h">6. Loans</h3>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Loans can be backed by your deposit, by XLM collateral worth 120% of the loan, or by up to 2 guarantors.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Your tier sets your limit. Guarantor backing doubles it.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Before you accept a loan, we show you its amount, interest, fees and repayment schedule.</li>
</ul>
</section>
<section id="dlg-t-repay" className="plr-dlg-sec" aria-labelledby="dlg-t-repay-h">
<h3 id="dlg-t-repay-h">7. Repayments and interest</h3>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Repayments are made with PayPal or Stripe, and their processing fee is added to each payment.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Interest is charged monthly on the principal you still owe: 2% to 1% for deposit or XLM-backed loans, 6% to 3% for guarantor-backed loans.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Late or missed payments can lower your credit score and future limits.</li>
</ul>
</section>
<section id="dlg-t-guar" className="plr-dlg-sec" aria-labelledby="dlg-t-guar-h">
<h3 id="dlg-t-guar-h">8. Guarantors</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>A guarantor’s pledged deposit is locked while the loan is active. Guarantors earn a share of that loan’s interest.</p>
</section>
<section id="dlg-t-use" className="plr-dlg-sec" aria-labelledby="dlg-t-use-h">
<h3 id="dlg-t-use-h">9. Acceptable use</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Don’t misuse the service, interfere with access, try unsupported access methods, upload harmful content, or disrupt other members. Don’t reverse engineer, overload or bypass platform protections.</p>
</section>
<section id="dlg-t-liab" className="plr-dlg-sec" aria-labelledby="dlg-t-liab-h">
<h3 id="dlg-t-liab-h">10. Liability</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>The service is provided as is. To the extent the law allows, PrimeLendRow isn’t liable for indirect or consequential losses from using it.</p>
</section>
<section id="dlg-t-changes" className="plr-dlg-sec" aria-labelledby="dlg-t-changes-h">
<h3 id="dlg-t-changes-h">11. Changes</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>We may update these terms. We’ll tell you when we do, and using the service after an update means you accept it.</p>
</section>
<section id="dlg-t-law" className="plr-dlg-sec" aria-labelledby="dlg-t-law-h">
<h3 id="dlg-t-law-h">12. Governing law</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>These terms are governed by the laws that apply where PrimeLendRow operates.</p>
</section>
<section id="dlg-t-contact" className="plr-dlg-sec" aria-labelledby="dlg-t-contact-h">
<h3 id="dlg-t-contact-h">13. Contact</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Questions about these terms? Reach us through support in the app.</p>
</section>
</div>
</>)}
{v.dlgIs.privacy && (<>
<div className="plr-dlg-pane">
<div>
<h2 id="plr-dlg-title" className="plr-dlg-title">Privacy <span style={{"color": "var(--landing-accent)"}}>Policy.</span></h2>
<p className="plr-dlg-sub">What we collect, why, and the choices you have.</p>
</div>
<section id="dlg-p-who" className="plr-dlg-sec" aria-labelledby="dlg-p-who-h">
<h3 id="dlg-p-who-h">1. Who we are</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>PrimeLendRow is responsible for the personal data you share with us and handles it in line with applicable data privacy laws.</p>
</section>
<section id="dlg-p-collect" className="plr-dlg-sec" aria-labelledby="dlg-p-collect-h">
<h3 id="dlg-p-collect-h">2. What we collect</h3>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Identity details read from your ID, plus the document scan and selfie used for verification.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Contact details, like your email address.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Your Varial wallet address.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Deposit, loan and repayment records.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Payment details, which are handled by PayPal or Stripe.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>Basic device and usage information that keeps the service working and secure.</li>
</ul>
</section>
<section id="dlg-p-why" className="plr-dlg-sec" aria-labelledby="dlg-p-why-h">
<h3 id="dlg-p-why-h">3. Why we use it</h3>
<ul style={{"margin": "0", "padding": "0", "listStyle": "none", "display": "flex", "flexDirection": "column", "gap": "10px", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>To verify who you are.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>To run deposits, loans, repayments and withdrawals.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>To prevent fraud and keep accounts secure.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>To send account messages such as verification codes and security alerts.</li>
<li style={{"position": "relative", "paddingLeft": "18px"}}><span aria-hidden="true" style={{"position": "absolute", "left": "2px", "top": "calc(24px / 2 - 3px)", "width": "6px", "height": "6px", "borderRadius": "50%", "background": "var(--landing-accent)"}}>

</span>To meet our legal obligations.</li>
</ul>
</section>
<section id="dlg-p-share" className="plr-dlg-sec" aria-labelledby="dlg-p-share-h">
<h3 id="dlg-p-share-h">4. Who we share it with</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>We never sell your data. We share it only with PayPal and Stripe to process payments, identity verification partners to confirm who you are, and regulators or authorities when the law requires it.</p>
</section>
<section id="dlg-p-keep" className="plr-dlg-sec" aria-labelledby="dlg-p-keep-h">
<h3 id="dlg-p-keep-h">5. How long we keep it</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>We keep your data only as long as we need it to run your account or as the law requires, then delete or anonymise it.</p>
</section>
<section id="dlg-p-rights" className="plr-dlg-sec" aria-labelledby="dlg-p-rights-h">
<h3 id="dlg-p-rights-h">6. Your rights</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>You can ask to see your data, correct it, delete it, object to how it’s used, or get a copy to take elsewhere. You can also complain to your data protection authority. To use these rights, contact support.</p>
</section>
<section id="dlg-p-sec" className="plr-dlg-sec" aria-labelledby="dlg-p-sec-h">
<h3 id="dlg-p-sec-h">7. Security</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>We use reasonable technical and organisational safeguards to protect your data. No system is perfectly secure, so tell us right away if you notice anything unusual.</p>
</section>
<section id="dlg-p-changes" className="plr-dlg-sec" aria-labelledby="dlg-p-changes-h">
<h3 id="dlg-p-changes-h">8. Changes</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>We’ll tell you when this policy changes. The latest version always lives on this page.</p>
</section>
<section id="dlg-p-contact" className="plr-dlg-sec" aria-labelledby="dlg-p-contact-h">
<h3 id="dlg-p-contact-h">9. Contact</h3>
<p style={{"margin": "0", "fontSize": "15px", "lineHeight": "24px", "color": "#bbc6d1"}}>Questions about privacy? Reach us through support in the app.</p>
</section>
</div>
</>)}
</div>
</div>
</div>
</>)}
    </>)
}
