import { useState } from 'react'
import { Calendar } from 'lucide-react'

const MONTHS = [
    'January', 'February', 'March', 'April', 'May', 'June',
    'July', 'August', 'September', 'October', 'November', 'December',
]

const pad2 = (n: number) => String(n).padStart(2, '0')
const daysInMonth = (month: number, year: number) => new Date(year, month, 0).getDate()

/** Parses a DD/MM/YYYY string (the format `dob` is stored in) into its parts, 1-indexed month. */
function parseDob(value: string): { day: number; month: number; year: number } | null {
    const m = value.trim().match(/^(\d{1,2})\/(\d{1,2})\/(\d{4})$/)
    if (!m) return null
    const day = Number(m[1]), month = Number(m[2]), year = Number(m[3])
    if (month < 1 || month > 12 || day < 1 || day > daysInMonth(month, year)) return null
    return { day, month, year }
}

type DobPickerProps = {
    value: string
    onChange: (value: string) => void
}

/**
 * A day/month/year picker for the KYC date-of-birth field. Three plain
 * <select>s rather than a calendar grid — for a birthdate decades in the
 * past, paging a calendar back month by month is far more clicks than
 * jumping straight to a year.
 */
export default function DobPicker({ value, onChange }: DobPickerProps) {
    const [open, setOpen] = useState(false)
    const now = new Date()
    const currentYear = now.getFullYear()

    const fallback = { day: now.getDate(), month: now.getMonth() + 1, year: currentYear - 30 }
    const [day, setDay] = useState(fallback.day)
    const [month, setMonth] = useState(fallback.month)
    const [year, setYear] = useState(fallback.year)

    const maxDay = daysInMonth(month, year)
    const clampedDay = Math.min(day, maxDay)
    const years = Array.from({ length: currentYear - 1900 + 1 }, (_, i) => currentYear - i)

    // Re-sync the pickers with whatever is already in the field (typed or
    // OCR-filled) right when the user opens the modal, so it never
    // contradicts what's already there. Done here, in the event that causes
    // `open` to become true, rather than in an effect reacting to `open` —
    // this is a response to a user action, not synchronization with an
    // external system, so it doesn't need an effect at all.
    const openPicker = () => {
        const parsed = parseDob(value) ?? fallback
        setDay(parsed.day)
        setMonth(parsed.month)
        setYear(parsed.year)
        setOpen(true)
    }

    const confirm = () => {
        onChange(`${pad2(clampedDay)}/${pad2(month)}/${year}`)
        setOpen(false)
    }

    return (
        <>
            <button
                type='button'
                className='kyc-dob-trigger'
                onClick={openPicker}
                aria-label='Pick date of birth from a calendar'
            >
                <Calendar />
            </button>

            {open && (
                <div className='kyc-modal' onClick={() => setOpen(false)}>
                    <div className='kyc-modal-card' onClick={e => e.stopPropagation()}>
                        <div className='kyc-modal-head'>
                            <div>
                                <h3>Date of birth</h3>
                                <p>Select the day, month, and year on your ID</p>
                            </div>
                            <button className='kyc-modal-close' type='button' aria-label='Close' onClick={() => setOpen(false)}>✕</button>
                        </div>

                        <div className='kyc-dob-body'>
                            <div className='kyc-dob-columns'>
                                <label>
                                    <span>Day</span>
                                    <select value={clampedDay} onChange={e => setDay(Number(e.target.value))}>
                                        {Array.from({ length: maxDay }, (_, i) => i + 1).map(d => (
                                            <option key={d} value={d}>{d}</option>
                                        ))}
                                    </select>
                                </label>
                                <label>
                                    <span>Month</span>
                                    <select value={month} onChange={e => setMonth(Number(e.target.value))}>
                                        {MONTHS.map((m, i) => (
                                            <option key={m} value={i + 1}>{m}</option>
                                        ))}
                                    </select>
                                </label>
                                <label>
                                    <span>Year</span>
                                    <select value={year} onChange={e => setYear(Number(e.target.value))}>
                                        {years.map(y => (
                                            <option key={y} value={y}>{y}</option>
                                        ))}
                                    </select>
                                </label>
                            </div>
                        </div>

                        <div className='kyc-modal-foot'>
                            <button type='button' className='kyc-btn-ghost' onClick={() => setOpen(false)}>Cancel</button>
                            <button type='button' className='kyc-btn-primary' onClick={confirm}>Use this date</button>
                        </div>
                    </div>
                </div>
            )}
        </>
    )
}
