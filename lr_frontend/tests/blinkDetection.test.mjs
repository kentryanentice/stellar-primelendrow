import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import ts from 'typescript'

// Transpile the production helper in memory so this suite also works on the
// Node 20 versions supported by Vite (native TypeScript stripping needs 22.6+).
const source = await readFile(new URL('../src/functions/KYC/blinkDetection.ts', import.meta.url), 'utf8')
const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
        module: ts.ModuleKind.ESNext,
        target: ts.ScriptTarget.ES2022,
    },
})
const {
    createBlinkTracker,
    eyeOpenness,
    resetBlinkTracker,
    updateBlinkTracker,
} = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`)

const calibrate = (left = 0.22, right = 0.34) => {
    const tracker = createBlinkTracker()
    for (let frame = 0; frame < 10; frame++) {
        updateBlinkTracker(tracker, [left, right], frame * 60)
    }
    assert.equal(tracker.phase, 'ready')
    return tracker
}

test('detects a blink relative to unequal per-eye baselines', () => {
    const tracker = calibrate()

    assert.equal(updateBlinkTracker(tracker, [0.1, 0.15], 700), 'closed')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], 820), 'detected')
})

test('accepts a one-frame blink at common 120, 60, and 30 fps intervals', () => {
    for (const frameDuration of [1000 / 120, 1000 / 60, 1000 / 30]) {
        const tracker = calibrate()
        assert.equal(updateBlinkTracker(tracker, [0.1, 0.15], 700), 'closed')
        assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], 700 + frameDuration), 'detected')
    }
})

test('does not accept a one-eye wink', () => {
    const tracker = calibrate()

    assert.equal(updateBlinkTracker(tracker, [0.06, 0.34], 700), 'closing')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], 760), 'ready')
})

test('allows natural left/right eyelid timing while the first eye remains closed', () => {
    const tracker = calibrate()

    assert.equal(updateBlinkTracker(tracker, [0.1, 0.34], 700), 'closing')
    assert.equal(updateBlinkTracker(tracker, [0.1, 0.15], 820), 'closed')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], 900), 'detected')
})

test('keeps an eye closed through the hysteresis band while the other catches up', () => {
    const tracker = calibrate()

    assert.equal(updateBlinkTracker(tracker, [0.1, 0.34], 700), 'closing')
    assert.equal(updateBlinkTracker(tracker, [0.175, 0.15], 780), 'closed')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], 840), 'detected')
})

test('accepts a bilateral blink when one eye has a shallower landmark dip', () => {
    const tracker = calibrate()

    assert.equal(updateBlinkTracker(tracker, [0.1, 0.24], 700), 'closed')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], 760), 'detected')
})

test('accepts an unambiguous bilateral closure after a slow processed frame', () => {
    const tracker = calibrate()

    assert.equal(updateBlinkTracker(tracker, [0.1, 0.34], 700), 'closing')
    assert.equal(updateBlinkTracker(tracker, [0.1, 0.15], 900), 'closed')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], 960), 'detected')
})

test('does not combine alternating non-overlapping winks into a blink', () => {
    const tracker = calibrate()

    assert.equal(updateBlinkTracker(tracker, [0.1, 0.34], 700), 'closing')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.15], 820), 'closing')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], 880), 'ready')
})

test('requires sustained open-looking samples before arming', () => {
    const tracker = createBlinkTracker()
    for (let frame = 0; frame < 8; frame++) {
        updateBlinkTracker(tracker, [0.06, 0.07], frame * 50)
    }
    assert.equal(tracker.phase, 'calibrating')

    for (let frame = 0; frame < 10; frame++) {
        updateBlinkTracker(tracker, [0.24, 0.3], 500 + frame * 60)
    }
    assert.equal(tracker.phase, 'ready')
})

test('keeps calibration progress through one jittery landmark sample', () => {
    const tracker = createBlinkTracker()
    for (let frame = 0; frame < 5; frame++) {
        updateBlinkTracker(tracker, [0.1, 0.16], frame * 60)
    }
    updateBlinkTracker(tracker, [0.06, 0.16], 300)
    for (let frame = 6; frame <= 10; frame++) {
        updateBlinkTracker(tracker, [0.1, 0.16], frame * 60)
    }
    assert.equal(tracker.phase, 'ready')
})

test('does not arm from a highly unstable calibration baseline', () => {
    const tracker = createBlinkTracker()
    for (let frame = 0; frame < 5; frame++) {
        updateBlinkTracker(tracker, [0.1, 0.1], frame * 60)
    }
    for (let frame = 5; frame < 10; frame++) {
        updateBlinkTracker(tracker, [0.5, 0.5], frame * 60)
    }
    assert.equal(tracker.phase, 'calibrating')

    for (let frame = 10; frame < 20; frame++) {
        updateBlinkTracker(tracker, [0.22, 0.3], frame * 60)
    }
    assert.equal(tracker.phase, 'ready')
    assert.deepEqual(tracker.baseline, [0.22, 0.3])
})

test('rejects implausibly short and long closures', () => {
    const tooFast = calibrate()
    assert.equal(updateBlinkTracker(tooFast, [0.1, 0.15], 700), 'closed')
    assert.equal(updateBlinkTracker(tooFast, [0.22, 0.34], 704), 'waiting-open')
    assert.equal(updateBlinkTracker(tooFast, [0.22, 0.34], 760), 'ready')

    const tooLong = calibrate()
    assert.equal(updateBlinkTracker(tooLong, [0.1, 0.15], 700), 'closed')
    assert.equal(updateBlinkTracker(tooLong, [0.22, 0.34], 1700), 'ready')
})

test('ignores duplicate camera timestamps and resets identity-bound state', () => {
    const tracker = calibrate()
    assert.equal(updateBlinkTracker(tracker, [0.1, 0.15], 700), 'closed')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], 700), 'closed')

    resetBlinkTracker(tracker)
    assert.equal(tracker.phase, 'calibrating')
    assert.equal(tracker.baseline, null)
})

test('ignores non-finite timestamps without poisoning later samples', () => {
    const tracker = createBlinkTracker()
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], Number.NaN), 'calibrating')
    assert.equal(updateBlinkTracker(tracker, [0.22, 0.34], Number.POSITIVE_INFINITY), 'calibrating')
    assert.equal(tracker.lastSampleAt, null)

    for (let frame = 0; frame < 10; frame++) {
        updateBlinkTracker(tracker, [0.22, 0.34], frame * 60)
    }
    assert.equal(tracker.phase, 'ready')
})

test('uses multiple lid landmarks for each eye aspect ratio', () => {
    const mesh = Array.from({ length: 468 }, () => [0, 0, 0])

    mesh[362] = [0, 0, 0]
    mesh[263] = [10, 0, 0]
    mesh[385] = [2, -2, 0]
    mesh[380] = [2, 2, 0]
    mesh[387] = [8, -2, 0]
    mesh[373] = [8, 2, 0]

    mesh[33] = [0, 0, 0]
    mesh[133] = [10, 0, 0]
    mesh[160] = [2, -1, 0]
    mesh[144] = [2, 1, 0]
    mesh[158] = [8, -1, 0]
    mesh[153] = [8, 1, 0]

    assert.deepEqual(eyeOpenness(mesh), [0.4, 0.2])
})

test('rejects a sparse or non-finite landmark mesh safely', () => {
    assert.equal(eyeOpenness(Array(468)), null)

    const mesh = Array.from({ length: 468 }, () => [0, 0, 0])
    mesh[362] = [Number.NaN, 0, 0]
    assert.equal(eyeOpenness(mesh), null)
})
