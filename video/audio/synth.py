"""Synthesise the walkthrough's ambient score and sound effects.

Everything is generated from code — no samples, no library — so the audio is
reproducible and carries no licence beyond the repository's own. Writes
16-bit stereo WAVs into ../public/audio/. Deterministic (fixed seed).

    python audio/synth.py
"""

import os
import wave

import numpy as np

SR = 44100
OUT = os.path.join(os.path.dirname(__file__), "..", "public", "audio")
rng = np.random.default_rng(5097)


def t_of(seconds):
    return np.arange(int(seconds * SR)) / SR


def write(name, x, peak=0.8):
    """Normalise to `peak` and write a stereo 16-bit WAV."""
    if x.ndim == 1:
        x = np.stack([x, x], axis=1)
    m = np.max(np.abs(x)) or 1.0
    x = x / m * peak
    data = (np.clip(x, -1, 1) * 32767).astype("<i2")
    os.makedirs(OUT, exist_ok=True)
    with wave.open(os.path.join(OUT, f"{name}.wav"), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(data.tobytes())


def lowpass(x, cutoff):
    """One-pole low-pass; `cutoff` is a number or a per-sample array (Hz)."""
    c = np.broadcast_to(np.asarray(cutoff, dtype=float), x.shape)
    a = 1 - np.exp(-2 * np.pi * c / SR)
    y = np.empty_like(x)
    s = 0.0
    for i in range(len(x)):
        s += a[i] * (x[i] - s)
        y[i] = s
    return y


def highpass(x, cutoff):
    return x - lowpass(x, cutoff)


def env(n, attack, release, curve=4.0):
    """Attack then exponential-ish release, both in seconds."""
    e = np.ones(n)
    a = max(1, int(attack * SR))
    e[:a] = np.linspace(0, 1, a)
    r = np.arange(n - a) / SR
    e[a:] = np.exp(-curve * r / max(release, 1e-3))
    return e


def pan(x, p):
    """Equal-power pan; p in [-1, 1], number or per-sample array."""
    p = np.broadcast_to(np.asarray(p, dtype=float), x.shape)
    ang = (p + 1) * np.pi / 4
    return np.stack([x * np.cos(ang), x * np.sin(ang)], axis=1)


def reverb(x, seconds=2.4, mix=0.3, seed=1):
    """Convolution with decaying stereo noise, by overlap-add."""
    r = np.random.default_rng(seed)
    n = int(seconds * SR)
    decay = np.exp(-6 * np.arange(n) / n)
    out = []
    for ch in range(2):
        ir = r.standard_normal(n) * decay
        ir = lowpass(ir, 5000) if n < SR * 3 else ir
        ir /= np.sqrt(np.sum(ir ** 2))
        src = x[:, ch]
        block = 1 << 17
        size = 1 << int(np.ceil(np.log2(block + n)))
        IR = np.fft.rfft(ir, size)
        y = np.zeros(len(src) + n)
        for s in range(0, len(src), block):
            seg = src[s : s + block]
            conv = np.fft.irfft(np.fft.rfft(seg, size) * IR, size)[: len(seg) + n]
            y[s : s + len(conv)] += conv
        out.append(y[: len(src)])
    wet = np.stack(out, axis=1)
    return x * (1 - mix) + wet * mix


def note(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


# ─── Sound effects ─────────────────────────────────────────────────────────

def key(i):
    """A soft mechanical key: a filtered click and a low thock."""
    t = t_of(0.07)
    click = highpass(rng.standard_normal(len(t)), 1800) * np.exp(-t / 0.004)
    thock = np.sin(2 * np.pi * (150 + 25 * i) * t) * np.exp(-t / 0.018)
    x = 0.55 * click + 0.6 * thock
    return pan(lowpass(x, 6000 + 400 * i), (i - 2.5) * 0.08)


def pop():
    """A node appearing: a short pitched drop."""
    t = t_of(0.12)
    f = 950 * np.exp(-t / 0.05) + 420
    x = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.03)
    return pan(x, 0)


def connect():
    """Two nodes joined: a rising fifth, bell-soft."""
    t = t_of(0.5)
    a = np.sin(2 * np.pi * 660 * t) * np.exp(-t / 0.08)
    b = np.sin(2 * np.pi * 990 * t) * np.exp(-np.clip(t - 0.06, 0, None) / 0.18) * (t > 0.06)
    shimmer = np.sin(2 * np.pi * 1980 * t) * 0.15 * np.exp(-t / 0.1)
    x = 0.6 * a + 0.7 * b + shimmer
    return reverb(pan(x, 0.1), 0.8, 0.25, 2)


def data():
    """A packet in flight: five quick rising blips."""
    steps = [0, 2, 4, 7, 9]
    t = t_of(0.32)
    x = np.zeros(len(t))
    for k, s in enumerate(steps):
        start = int(k * 0.045 * SR)
        tt = t[: len(t) - start]
        blip = np.sin(2 * np.pi * note(84 + s) * tt) * np.exp(-tt / 0.02)
        x[start:] += blip * (0.6 + 0.1 * k)
    return reverb(pan(x, np.linspace(-0.4, 0.4, len(x))), 0.6, 0.2, 3)


def chime():
    """A highlight or a found thing: an inharmonic bell."""
    t = t_of(1.8)
    f0 = 1046.5
    x = sum(a * np.sin(2 * np.pi * f0 * r * t) * np.exp(-t / d) for r, a, d in [(1, 1, 0.9), (2.76, 0.4, 0.4), (5.4, 0.18, 0.2), (0.5, 0.25, 1.2)])
    return reverb(pan(x, -0.15), 1.6, 0.35, 4)


def whoosh(seconds, lo, hi, name_seed):
    """Air moving past: noise through a sweeping low-pass, panned across."""
    r = np.random.default_rng(name_seed)
    t = t_of(seconds)
    u = t / seconds
    shape = np.sin(np.pi * u) ** 1.6
    cutoff = lo + (hi - lo) * np.sin(np.pi * np.clip(u * 1.1, 0, 1)) ** 2
    x = lowpass(r.standard_normal(len(t)), cutoff) * shape
    x = highpass(x, 120)
    return reverb(pan(x, np.linspace(-0.6, 0.6, len(t))), 1.2, 0.3, name_seed)


def swish():
    """The halftone sweep: a brighter, quicker brush of noise."""
    t = t_of(0.75)
    u = t / 0.75
    x = highpass(rng.standard_normal(len(t)), 2500) * np.sin(np.pi * u) ** 2
    x = lowpass(x, 3000 + 6000 * u)
    return reverb(pan(x, np.linspace(0.7, -0.7, len(t))), 0.9, 0.3, 6)


def thump():
    """A soft impact under a line landing."""
    t = t_of(0.9)
    f = 90 * np.exp(-t / 0.08) + 48
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.22)
    click = lowpass(rng.standard_normal(len(t)), 1500) * np.exp(-t / 0.01) * 0.3
    return reverb(pan(body + click, 0), 1.4, 0.25, 7)


def riser(seconds=1.4):
    t = t_of(seconds)
    u = t / seconds
    f = 220 * 2 ** (2 * u)
    tone = np.sin(2 * np.pi * np.cumsum(f) / SR) * 0.25
    air = lowpass(rng.standard_normal(len(t)), 800 + 7000 * u) * 0.6
    x = (tone + air) * u ** 2
    return reverb(pan(x, 0), 1.2, 0.3, 8)


def shimmer(seconds=3.2):
    """The dot field filling: a rain of tiny high blips."""
    t = t_of(seconds)
    x = np.zeros(len(t))
    for k in range(260):
        at = rng.uniform(0, seconds - 0.05) ** 1.15 / seconds ** 0.15
        s = int(at * SR)
        n = int(0.025 * SR)
        if s + n >= len(x):
            continue
        tt = np.arange(n) / SR
        x[s : s + n] += np.sin(2 * np.pi * note(rng.choice([88, 91, 93, 95, 98])) * tt) * np.exp(-tt / 0.008) * rng.uniform(0.3, 1)
    x *= np.minimum(1, (seconds - t) / 0.4)
    return reverb(pan(x, rng.uniform(-0.3, 0.3)), 1.4, 0.4, 9)


def fall():
    """The unverified dots going hollow: a gentle downward glide."""
    t = t_of(0.9)
    f = 520 * 2 ** (-1.4 * t)
    x = np.sin(2 * np.pi * np.cumsum(f) / SR) * env(len(t), 0.02, 0.8, 3)
    return reverb(pan(x, 0.2), 1.4, 0.35, 10)


def deny():
    """A refusal: two soft low blips."""
    t = t_of(0.4)
    x = np.zeros(len(t))
    for k, m in enumerate([62, 57]):
        s = int(k * 0.14 * SR)
        tt = t[: len(t) - s]
        x[s:] += np.sin(2 * np.pi * note(m) * tt) * np.exp(-tt / 0.06)
    return reverb(pan(x, 0), 0.8, 0.2, 11)


# ─── Ambient score ─────────────────────────────────────────────────────────

def score(seconds=150.0, bpm=96):
    """A slow pad in D over four chords, a sub, and a soft pluck arpeggio
    that enters after the opening and leaves before the end."""
    t = t_of(seconds)
    n = len(t)
    chords = [
        [50, 57, 62, 66, 69, 73, 76],  # Dmaj9
        [47, 54, 59, 62, 66, 69, 76],  # Bm11
        [43, 50, 55, 59, 62, 66, 69],  # Gmaj9
        [45, 52, 57, 62, 64, 69, 74],  # Asus
    ]
    bar = 8.0
    pad = np.zeros((n, 2))
    sub = np.zeros(n)
    for c in range(int(np.ceil(seconds / bar)) + 1):
        chord = chords[c % len(chords)]
        start = c * bar - 1.5
        s0 = max(0, int(start * SR))
        s1 = min(n, int((start + bar + 3.0) * SR))
        if s1 <= s0:
            continue
        tt = t[s0:s1] - start
        dur = bar + 3.0
        e = np.clip(tt / 2.5, 0, 1) * np.clip((dur - tt) / 2.5, 0, 1)
        for k, m in enumerate(chord[1:]):
            f = note(m)
            for side, det in ((0, -0.18), (1, 0.18)):
                v = np.sin(2 * np.pi * (f + det * (k + 1) * 0.6) * tt + k)
                v += 0.18 * np.sin(2 * np.pi * 2 * (f + det) * tt)
                trem = 1 + 0.12 * np.sin(2 * np.pi * (0.11 + 0.03 * k) * tt + k)
                pad[s0:s1, side] += v * e * trem * (0.16 / (1 + 0.25 * k))
        root = note(chord[0] - 12)
        sub[s0:s1] += np.sin(2 * np.pi * root * tt) * e * 0.35

    # Pluck arpeggio, in from ~6 s, out ~8 s before the end, with a ping-pong delay.
    step = 60 / bpm / 2
    pl = np.zeros((n, 2))
    pattern = [0, 2, 4, 6, 4, 2, 3, 5]
    k = 0
    tp = 6.0
    while tp < seconds - 8:
        chord = chords[int(tp // bar) % len(chords)]
        m = chord[1 + pattern[k % len(pattern)] % (len(chord) - 1)] + 12
        s = int(tp * SR)
        ln = int(0.6 * SR)
        tt = np.arange(min(ln, n - s)) / SR
        f = note(m)
        v = (np.sin(2 * np.pi * f * tt) + 0.3 * np.sin(2 * np.pi * 2 * f * tt) + 0.1 * np.sin(2 * np.pi * 3 * f * tt)) * np.exp(-tt / 0.16)
        gate = min(1, (tp - 6) / 8) * min(1, (seconds - 8 - tp) / 6)
        vel = (0.55 + 0.25 * ((k % 4) == 0)) * gate
        pl[s : s + len(tt)] += pan(v, 0.35 if k % 2 else -0.35) * vel * 0.32
        k += 1
        tp += step
    d = int(step * 3 * SR)
    echo = np.zeros_like(pl)
    echo[d:, 0] += pl[:-d, 1] * 0.38
    echo[d:, 1] += pl[:-d, 0] * 0.38
    echo[2 * d :, :] += pl[: -2 * d, :] * 0.16
    pl = pl + echo

    # Air: very quiet filtered noise, slowly breathing.
    air = lowpass(rng.standard_normal(n) * 0.02, 900) * (1 + 0.5 * np.sin(2 * np.pi * t / 13))

    mix = pad + pan(sub, 0) + pl + pan(air, 0)
    mix = reverb(mix, 3.2, 0.38, 12)
    fade = np.minimum(1, t / 3) * np.minimum(1, (seconds - t) / 4)
    return mix * fade[:, None]


# ─── The commercial: a 90s machine, then a quiet cinematic score ───────────

def square(f, t, duty=0.5):
    return np.where((f * t) % 1 < duty, 1.0, -1.0)


def static(seconds=0.8):
    """Tape tracking noise at the start of the tape."""
    t = t_of(seconds)
    x = highpass(rng.standard_normal(len(t)), 600) * (0.6 + 0.4 * (rng.random(len(t)) > 0.97))
    x *= np.exp(-t / (seconds * 0.5))
    return pan(lowpass(x, 7000), 0)


def hum(seconds=20.0):
    """A CRT in a quiet room: mains hum and a little hiss, loopable."""
    t = t_of(seconds)
    x = 0.5 * np.sin(2 * np.pi * 60 * t) + 0.3 * np.sin(2 * np.pi * 120 * t) + 0.12 * np.sin(2 * np.pi * 180 * t)
    x += lowpass(rng.standard_normal(len(t)) * 0.08, 3000)
    x *= 1 + 0.05 * np.sin(2 * np.pi * 0.3 * t)
    return pan(x, 0)


def beep(freq=880, seconds=0.16):
    """A PC speaker beep."""
    t = t_of(seconds)
    x = square(freq, t) * env(len(t), 0.002, seconds, 0.6)
    x[-int(0.01 * SR):] *= np.linspace(1, 0, int(0.01 * SR))
    return pan(lowpass(x, 4000), 0)


def hdd(seconds=0.8):
    """A hard disk seeking: an irregular run of small clicks."""
    t = t_of(seconds)
    x = np.zeros(len(t))
    at = 0.0
    while at < seconds - 0.03:
        s = int(at * SR)
        n = int(0.012 * SR)
        tt = np.arange(n) / SR
        x[s : s + n] += highpass(rng.standard_normal(n), 1500) * np.exp(-tt / 0.002) * rng.uniform(0.4, 1)
        at += rng.uniform(0.02, 0.09)
    x += lowpass(rng.standard_normal(len(t)), 400) * 0.08
    return pan(x, 0.25)


def crt_off():
    """The set switched off: a falling zap, a click, a static tail."""
    t = t_of(1.1)
    f = 1400 * np.exp(-t / 0.08) + 50
    zap = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.18)
    click = lowpass(rng.standard_normal(len(t)), 2500) * np.exp(-t / 0.006)
    tail = highpass(rng.standard_normal(len(t)), 3000) * np.exp(-t / 0.25) * 0.15
    return reverb(pan(zap + 0.6 * click + tail, 0), 1.0, 0.25, 13)


def retro_tune(seconds=11.7, layer2=3.67, layer3=7.67):
    """A small square-wave tune that gains a voice at each era, and stops dead."""
    t = t_of(seconds)
    n = len(t)
    bpm = 120
    eighth = 60 / bpm / 2
    lead = [69, 72, 76, 72, 74, 72, 69, 67, 69, 72, 76, 79, 77, 76, 74, 72]
    bass = [45, 45, 52, 52, 41, 41, 48, 48]
    x = np.zeros(n)
    for k in range(int(seconds / eighth)):
        s = int(k * eighth * SR)
        ln = min(int(eighth * SR * 0.85), n - s)
        if ln <= 0:
            break
        tt = np.arange(ln) / SR
        e = np.exp(-tt / 0.12)
        x[s : s + ln] += square(note(lead[k % len(lead)]), tt, 0.25) * e * 0.22
        if k * eighth >= layer2 and k % 2 == 0:
            x[s : s + ln] += square(note(bass[(k // 2) % len(bass)]), tt, 0.5) * np.exp(-tt / 0.2) * 0.2
        if k * eighth >= layer3:
            hat = highpass(rng.standard_normal(ln), 6000) * np.exp(-tt / 0.01) * 0.12
            x[s : s + ln] += hat
            x[s : s + ln] += np.sin(2 * np.pi * note(lead[(k + 3) % len(lead)] + 12) * tt) * np.exp(-tt / 0.06) * 0.08
    x = lowpass(x, 5000)
    x *= np.minimum(1, t / 0.5)
    return reverb(pan(x, 0), 0.9, 0.15, 14)


def piano(m, seconds=4.0, vel=1.0):
    """A soft, slightly inharmonic struck string."""
    t = t_of(seconds)
    f = note(m)
    x = np.zeros(len(t))
    for h, (a, d) in enumerate([(1, 2.2), (0.5, 1.4), (0.25, 0.9), (0.12, 0.6), (0.06, 0.4)], start=1):
        fh = f * h * (1 + 0.0004 * h * h)
        x += a * np.sin(2 * np.pi * fh * t) * np.exp(-t / d)
    x *= np.minimum(1, t / 0.004) * vel
    return x


def cinema(seconds=64.4, solution=12.33, how=22.67, pros=38.0, vision=49.33, card=56.0):
    """The modern half: a low drone, sparse piano, a swell at the solution,
    a soft pulse under how it works, and a resolving chord on the end card."""
    t = t_of(seconds)
    n = len(t)
    mix = np.zeros((n, 2))

    # Drone on D, opening up from a filtered hum.
    drone = 0.5 * np.sin(2 * np.pi * note(38) * t) + 0.25 * np.sin(2 * np.pi * note(45) * t + 1) + 0.12 * np.sin(2 * np.pi * note(50) * t + 2)
    drone *= np.clip(t / 4, 0, 1) * (0.8 + 0.2 * np.sin(2 * np.pi * t / 9))
    mix += pan(drone * 0.5, 0)

    def place(x, at, p=0.0, gain=1.0):
        s = int(at * SR)
        if s >= n:
            return
        x = x[: n - s]
        mix[s : s + len(x)] += pan(x, p) * gain

    # Sparse piano through the problem: questions, not answers.
    for at, m, p in [(1.5, 62, -0.2), (4.0, 69, 0.2), (6.5, 65, -0.1), (9.0, 64, 0.15), (10.8, 57, 0)]:
        place(piano(m, 4.5, 0.55), at, p)

    # The solution: a pad swells in (Dmaj9), held under the rest.
    pad_notes = [50, 57, 62, 66, 69, 73, 76]
    pad = np.zeros(n)
    tt = t - solution
    e = np.clip(tt / 3.0, 0, 1) * np.clip((seconds - t) / 3.0, 0, 1) * (tt > 0)
    for k, m in enumerate(pad_notes):
        pad += np.sin(2 * np.pi * note(m) * t + k) * (0.12 / (1 + 0.3 * k))
        pad += np.sin(2 * np.pi * (note(m) + 0.3) * t + 2 * k) * (0.08 / (1 + 0.3 * k))
    mix += pan(pad * e, 0) * 0.9

    # How it works: a heartbeat pulse and a rising piano arpeggio.
    beat = 60 / 84
    at = how
    k = 0
    arp = [62, 66, 69, 73, 74, 73, 69, 66]
    while at < pros + 0.5:
        tk = t_of(0.5)
        kick = np.sin(2 * np.pi * np.cumsum(55 * np.exp(-tk / 0.06) + 42) / SR) * np.exp(-tk / 0.18)
        place(kick, at, 0, 0.55 * min(1, (at - how) / 4 + 0.3))
        place(piano(arp[k % len(arp)] + 12, 2.0, 0.3), at + beat / 2, 0.3 if k % 2 else -0.3)
        k += 1
        at += beat

    # Pros: lighter — single high notes.
    for i, at in enumerate(np.arange(pros + 1.0, vision - 0.5, 2.3)):
        place(piano([78, 74, 81, 76, 73][i % 5], 3.5, 0.35), at, 0.25 if i % 2 else -0.25)

    # Vision: the chord moves (Bm9 → Gmaj9), then resolves to D on the card.
    for at, chord in [(vision, [47, 54, 59, 62, 66, 69]), (vision + 3.3, [43, 50, 55, 59, 62, 66])]:
        for j, m in enumerate(chord):
            place(piano(m, 5.0, 0.45), at + j * 0.05, (j - 2.5) * 0.12)
    for j, m in enumerate([38, 50, 57, 62, 66, 69, 74]):
        place(piano(m, 8.0, 0.6), card + j * 0.07, (j - 3) * 0.1)
    # A last swell of air under the card.
    air = lowpass(rng.standard_normal(n), 1200) * 0.05 * np.clip((t - card + 1) / 2, 0, 1) * np.clip((seconds - t) / 4, 0, 1)
    mix += pan(air, 0)

    mix = reverb(mix, 3.6, 0.42, 15)
    fade = np.minimum(1, t / 1.5) * np.minimum(1, (seconds - t) / 3)
    return mix * fade[:, None]


def commercial():
    write("static", static(), 0.6)
    write("hum", hum(), 0.5)
    write("beep", beep(), 0.5)
    write("beep-low", beep(440, 0.12), 0.45)
    write("hdd", hdd(), 0.55)
    write("crt-off", crt_off(), 0.8)
    write("retro-tune", retro_tune(), 0.6)
    write("cinema", cinema(), 0.95)


if __name__ == "__main__":
    commercial()
    for i in range(6):
        write(f"key{i}", key(i), 0.5)
    write("pop", pop(), 0.55)
    write("connect", connect(), 0.6)
    write("data", data(), 0.5)
    write("chime", chime(), 0.6)
    write("whoosh-short", whoosh(0.55, 400, 5000, 21), 0.7)
    write("whoosh", whoosh(0.95, 250, 6000, 22), 0.75)
    write("whoosh-long", whoosh(1.5, 150, 7000, 23), 0.8)
    write("swish", swish(), 0.55)
    write("thump", thump(), 0.8)
    write("riser", riser(), 0.6)
    write("shimmer", shimmer(), 0.5)
    write("fall", fall(), 0.55)
    write("deny", deny(), 0.55)
    write("score", score(), 0.7)
    print("wrote", sorted(os.listdir(OUT)))
