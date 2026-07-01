export const EyeIcon = ({ off }: { off: boolean }) => off ? (
    <svg width='19' height='19' viewBox='0 0 24 24' fill='none' stroke='currentColor' strokeWidth='2' strokeLinecap='round' strokeLinejoin='round'><path d='M9.9 5A10.4 10.4 0 0 1 12 5c6.5 0 10 7 10 7a18.5 18.5 0 0 1-3 3.7M6.6 6.6A18.4 18.4 0 0 0 2 12s3.5 7 10 7a10 10 0 0 0 4.2-.9' /><path d='m3 3 18 18' /><path d='M9.9 9.9a3 3 0 0 0 4.2 4.2' /></svg>
) : (
    <svg width='19' height='19' viewBox='0 0 24 24' fill='none' stroke='currentColor' strokeWidth='2' strokeLinecap='round' strokeLinejoin='round'><path d='M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z' /><circle cx='12' cy='12' r='3' /></svg>
)

export const CheckIcon = () => (
    <svg width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='currentColor' strokeWidth='3' strokeLinecap='round' strokeLinejoin='round'><path d='M20 6 9 17l-5-5' /></svg>
)

export const CrossIcon = () => (
    <svg width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='currentColor' strokeWidth='3' strokeLinecap='round' strokeLinejoin='round'><path d='M18 6 6 18M6 6l12 12' /></svg>
)

export const MailIcon = () => (
    <svg width='30' height='30' viewBox='0 0 24 24' fill='none' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' strokeLinejoin='round'><rect x='2' y='4' width='20' height='16' rx='2' /><path d='m2 7 10 6 10-6' /></svg>
)

export const ShieldIcon = () => (
    <svg width='15' height='15' viewBox='0 0 24 24' fill='none' stroke='currentColor' strokeWidth='1.8' strokeLinecap='round' strokeLinejoin='round'><path d='M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z' /><path d='m9 12 2 2 4-4' /></svg>
)

export const StarIcon = () => (
    <svg width='15' height='15' viewBox='0 0 24 24' fill='currentColor'><path d='m12 2 3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14l-5-4.87 6.91-1.01L12 2Z' /></svg>
)
