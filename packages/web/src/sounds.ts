// Short interface sounds, synthesised so they need no files and work offline.
// Each one repeats something already shown on screen; none is the only signal.
export type Sound =
  | "flip"
  | "file"
  | "right"
  | "miss"
  | "done"
  | "keep"
  | "send"
  | "rest"
  | "alert"
  | "tick";

// A device preference, like reminders: it applies to every profile in this browser.
const storageKey = "keepsake.sounds";

let enabled = (() => {
  try {
    return localStorage.getItem(storageKey) !== "off";
  } catch {
    return true;
  }
})();
let audio: AudioContext | null = null;

export function soundsOn(): boolean {
  return enabled;
}

export function setSoundsOn(on: boolean) {
  enabled = on;
  try {
    localStorage.setItem(storageKey, on ? "on" : "off");
  } catch {
    /* Without storage, the choice lasts until Keepsake reloads. */
  }
  if (on) unlock();
}

/** Browsers start audio only during a tap, so sounds that follow a save need it running already. */
function unlock() {
  if (!enabled) return;
  try {
    if (!audio) {
      // Ambient audio obeys the iPhone silent switch and mixes with the learner's music.
      const session = (
        navigator as Navigator & { audioSession?: { type: string } }
      ).audioSession;
      if (session) session.type = "ambient";
      audio = new AudioContext();
    }
    // iOS also interrupts audio when the app goes to the background.
    if (audio.state !== "running") void audio.resume().catch(() => undefined);
  } catch {
    /* This browser cannot play sounds. */
  }
}

/** Starts audio on taps and key presses. Returns a function that stops listening. */
export function unlockSoundsOnTap(): () => void {
  addEventListener("click", unlock, true);
  addEventListener("keydown", unlock, true);
  return () => {
    removeEventListener("click", unlock, true);
    removeEventListener("keydown", unlock, true);
  };
}

export function play(sound: Sound) {
  if (!enabled) return;
  // Browser tests listen for this instead of hearing the sound.
  dispatchEvent(new CustomEvent("keepsake:sound", { detail: sound }));
  if (!audio || audio.state === "closed") return;
  try {
    voices[sound](audio, audio.currentTime + 0.01);
  } catch {
    /* Sounds are optional. */
  }
}

type Voice = (audio: BaseAudioContext, at: number) => void;

function envelope(
  audio: BaseAudioContext,
  at: number,
  attack: number,
  duration: number,
  peak: number,
): GainNode {
  const gain = new GainNode(audio, { gain: 0 });
  gain.gain.setValueAtTime(0.0001, at);
  gain.gain.exponentialRampToValueAtTime(peak, at + attack);
  gain.gain.exponentialRampToValueAtTime(0.0001, at + duration);
  gain.connect(audio.destination);
  return gain;
}

/** A note that fades out, optionally gliding to another pitch. */
function tone(
  audio: BaseAudioContext,
  at: number,
  {
    frequency,
    to,
    duration,
    peak,
    wave = "sine",
  }: {
    frequency: number;
    to?: number;
    duration: number;
    peak: number;
    wave?: OscillatorType;
  },
) {
  const oscillator = new OscillatorNode(audio, { type: wave, frequency });
  oscillator.frequency.setValueAtTime(frequency, at);
  if (to) oscillator.frequency.exponentialRampToValueAtTime(to, at + duration);
  oscillator.connect(envelope(audio, at, 0.004, duration, peak));
  oscillator.start(at);
  oscillator.stop(at + duration);
}

/** Filtered noise, for paper, wood and air rather than notes. */
function noise(
  audio: BaseAudioContext,
  at: number,
  {
    filter,
    frequency,
    to,
    duration,
    peak,
    attack = 0.004,
  }: {
    filter: BiquadFilterType;
    frequency: number;
    to?: number;
    duration: number;
    peak: number;
    attack?: number;
  },
) {
  const buffer = new AudioBuffer({
    length: Math.ceil(audio.sampleRate * duration),
    sampleRate: audio.sampleRate,
  });
  const samples = buffer.getChannelData(0);
  for (let index = 0; index < samples.length; index++) {
    samples[index] = Math.random() * 2 - 1;
  }
  const source = new AudioBufferSourceNode(audio, { buffer });
  const shape = new BiquadFilterNode(audio, { type: filter, frequency });
  shape.frequency.setValueAtTime(frequency, at);
  if (to) shape.frequency.exponentialRampToValueAtTime(to, at + duration);
  source.connect(shape).connect(envelope(audio, at, attack, duration, peak));
  source.start(at);
}

// Phone speakers barely play below 200 Hz, so low sounds carry overtones or noise above it.
const voices: Record<Sound, Voice> = {
  // A card turning over.
  flip: (audio, at) =>
    noise(audio, at, {
      filter: "bandpass",
      frequency: 1200,
      to: 4000,
      duration: 0.14,
      peak: 0.3,
      attack: 0.03,
    }),
  // A card set down in a box. Every rating sounds alike, so none feels like a reward.
  file: (audio, at) => {
    noise(audio, at, {
      filter: "lowpass",
      frequency: 1400,
      duration: 0.07,
      peak: 0.25,
    });
    tone(audio, at, { frequency: 220, to: 150, duration: 0.09, peak: 0.15 });
  },
  right: (audio, at) => {
    tone(audio, at, {
      frequency: 659.25,
      duration: 0.18,
      peak: 0.1,
      wave: "triangle",
    });
    tone(audio, at + 0.09, {
      frequency: 987.77,
      duration: 0.3,
      peak: 0.1,
      wave: "triangle",
    });
  },
  // A wooden knock rather than a buzzer. The answer shown afterwards does the correcting.
  miss: (audio, at) => {
    tone(audio, at, {
      frequency: 330,
      to: 247,
      duration: 0.15,
      peak: 0.2,
      wave: "triangle",
    });
    noise(audio, at, {
      filter: "bandpass",
      frequency: 1800,
      duration: 0.02,
      peak: 0.1,
    });
  },
  done: (audio, at) => {
    [523.25, 659.25, 783.99].forEach((frequency, index) =>
      tone(audio, at + index * 0.08, {
        frequency,
        duration: 0.6,
        peak: 0.08,
        wave: "triangle",
      }),
    );
  },
  // A box lid closing: a low knock and its echo.
  keep: (audio, at) => {
    tone(audio, at, {
      frequency: 196,
      to: 131,
      duration: 0.2,
      peak: 0.22,
      wave: "triangle",
    });
    tone(audio, at + 0.07, {
      frequency: 262,
      to: 196,
      duration: 0.12,
      peak: 0.12,
      wave: "triangle",
    });
    noise(audio, at, {
      filter: "lowpass",
      frequency: 900,
      duration: 0.08,
      peak: 0.15,
    });
  },
  send: (audio, at) =>
    noise(audio, at, {
      filter: "bandpass",
      frequency: 500,
      to: 3500,
      duration: 0.35,
      peak: 0.25,
      attack: 0.15,
    }),
  // A bell with a bell's uneven overtones, so it sounds like a pause rather than a prize.
  rest: (audio, at) => {
    tone(audio, at, { frequency: 659.25, duration: 1.6, peak: 0.08 });
    tone(audio, at, { frequency: 659.25 * 2.76, duration: 0.6, peak: 0.02 });
    tone(audio, at, { frequency: 659.25 * 5.4, duration: 0.3, peak: 0.008 });
  },
  alert: (audio, at) => {
    tone(audio, at, {
      frequency: 392,
      duration: 0.16,
      peak: 0.12,
      wave: "triangle",
    });
    tone(audio, at + 0.17, {
      frequency: 311.13,
      duration: 0.22,
      peak: 0.12,
      wave: "triangle",
    });
  },
  tick: (audio, at) =>
    tone(audio, at, { frequency: 1400, duration: 0.03, peak: 0.04 }),
};
