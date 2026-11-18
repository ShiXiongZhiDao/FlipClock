/**
 * 翻页音效：Web Audio 现场合成，不打包任何音频文件。
 *
 * 音色拆成两段，与翻页动画的两个阶段对齐：
 *   1. t            机械弹片释放的「咔」——三角波 1200→140Hz + 带通噪声摩擦
 *   2. t + LAND_DELAY  叶片落到挡条的「咚」——正弦 260→45Hz + 1900Hz 脆响
 *
 * LAND_DELAY 取 260ms，对应上片折平、下片开始落下的时刻。调这个数必须和
 * `FlipDigit` 的 halfDuration 一起看，否则声音会早于或晚于画面。
 */

/** 落片声相对弹片声的延迟（秒）。 */
const LAND_DELAY = 0.26;

export type FlipSoundResult = "played" | "muted" | "unsupported";

let context: AudioContext | null = null;

function ensureContext(): AudioContext | null {
  if (context) return context;
  const Ctor =
    window.AudioContext ??
    (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Ctor) return null;
  try {
    context = new Ctor();
  } catch {
    return null;
  }
  return context;
}

/** 衰减噪声：翻页声里的「摩擦」和「脆响」都是它，只是滤波频段不同。 */
function noiseBuffer(audio: AudioContext, seconds: number, decay: number): AudioBuffer {
  const length = Math.max(1, Math.floor(audio.sampleRate * seconds));
  const buffer = audio.createBuffer(1, length, audio.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < length; i += 1) {
    data[i] = (Math.random() * 2 - 1) * Math.exp(-i / (length * decay));
  }
  return buffer;
}

/**
 * 在第一次用户输入时调用。WebView2 的音频上下文在没人碰过页面时是 suspended 的，
 * 屏保/全屏时钟里永远不会有点击，所以窗口创建时已经放开自动播放策略
 * （见 screensaver.rs 的 additional_browser_args）；这里只兜底普通浏览器预览。
 */
export function unlockFlipSound(): void {
  const audio = ensureContext();
  if (audio && audio.state === "suspended") void audio.resume().catch(() => {});
}

/**
 * 播一次翻页声。返回三种结果而不是布尔值：
 * `muted` 表示音量为 0（正常静音），`unsupported` 才是真的播不出来，
 * 调用方可以据此提示用户，而不是静默失败。
 */
export function playFlipSound(volume: number): FlipSoundResult {
  const master = Math.min(1, Math.max(0, volume));
  if (master <= 0) return "muted";

  const audio = ensureContext();
  if (!audio) return "unsupported";
  if (audio.state === "suspended") void audio.resume().catch(() => {});

  try {
    const t = audio.currentTime;

    // 阶段一：弹片释放的「咔」
    const click = audio.createOscillator();
    const clickGain = audio.createGain();
    const clickFilter = audio.createBiquadFilter();

    click.type = "triangle";
    click.frequency.setValueAtTime(1200, t);
    click.frequency.exponentialRampToValueAtTime(140, t + 0.025);

    clickFilter.type = "highpass";
    clickFilter.frequency.setValueAtTime(800, t);

    clickGain.gain.setValueAtTime(master * 0.45, t);
    clickGain.gain.exponentialRampToValueAtTime(0.001, t + 0.025);

    click.connect(clickFilter).connect(clickGain).connect(audio.destination);
    click.start(t);
    click.stop(t + 0.03);

    const rub = audio.createBufferSource();
    rub.buffer = noiseBuffer(audio, 0.02, 0.3);

    const rubFilter = audio.createBiquadFilter();
    rubFilter.type = "bandpass";
    rubFilter.frequency.setValueAtTime(2800, t);
    rubFilter.Q.setValueAtTime(3.5, t);

    const rubGain = audio.createGain();
    rubGain.gain.setValueAtTime(master * 0.35, t);
    rubGain.gain.exponentialRampToValueAtTime(0.001, t + 0.02);

    rub.connect(rubFilter).connect(rubGain).connect(audio.destination);
    rub.start(t);

    // 阶段二：叶片落到挡条的「咚」
    const land = t + LAND_DELAY;

    const thud = audio.createOscillator();
    const thudGain = audio.createGain();

    thud.type = "sine";
    thud.frequency.setValueAtTime(260, land);
    thud.frequency.exponentialRampToValueAtTime(45, land + 0.045);

    thudGain.gain.setValueAtTime(master * 0.65, land);
    thudGain.gain.exponentialRampToValueAtTime(0.001, land + 0.045);

    thud.connect(thudGain).connect(audio.destination);
    thud.start(land);
    thud.stop(land + 0.05);

    const snap = audio.createBufferSource();
    snap.buffer = noiseBuffer(audio, 0.035, 0.25);

    const snapFilter = audio.createBiquadFilter();
    snapFilter.type = "bandpass";
    snapFilter.frequency.setValueAtTime(1900, land);
    snapFilter.Q.setValueAtTime(2.8, land);

    const snapGain = audio.createGain();
    snapGain.gain.setValueAtTime(master * 0.55, land);
    snapGain.gain.exponentialRampToValueAtTime(0.001, land + 0.035);

    snap.connect(snapFilter).connect(snapGain).connect(audio.destination);
    snap.start(land);
  } catch (e) {
    console.warn("翻页音效播放失败", e);
    return "unsupported";
  }

  return "played";
}
