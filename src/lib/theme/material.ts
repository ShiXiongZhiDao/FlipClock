/**
 * 把一份 ThemeSpec 翻译成一套 CSS 变量（ADR-0020）。
 *
 * 这是「积木」的核心：用户只选材质与颜色，明暗、描边、投影、渐变分段
 * 全部由这里推导。材质决定光影结构，颜色只决定色相。
 */
import type {
  BackgroundSpec,
  DigitSpec,
  FlatPart,
  MaterialPart,
  ThemeSpec,
} from "../../types/settings";
import {
  compositeOver,
  darken,
  hexToRgb,
  lighten,
  luminance,
  swingScale,
  withAlpha,
} from "./color";

type Vars = Record<string, string>;

export interface ThemeAssetUrls {
  /** 已转成可加载地址的背景图。 */
  background?: string | null;
  /** 已转成可加载地址的数字纹理图。 */
  digit?: string | null;
}

const alphaOf = (part: { opacity: number }) =>
  Math.max(0, Math.min(100, part.opacity)) / 100;

/**
 * 卡面实际呈现的颜色。
 *
 * 数字压在卡片上，比的是渲染后的表面，不是用户选的基色：金属卡最亮的一道高光
 *（`cardVars` 里 `--card-top-fill` 的首段）被提亮 0.66 档，拿基色去比会把亮卡上的
 * 浅色数字误判成「凸起」。这里取六段折返的**均值**约 0.13 档——数字坐在卡面中段，
 * 不是坐在高光上；用顶面亮度当基准会把中段偏暗的卡判反。
 * 卡片带透明度时还要按透明度合成到背景上——0% 的卡片等于没有卡，数字直接坐在背景上。
 */
export function cardSurfaceColor(card: MaterialPart, background: string): string {
  const surface =
    card.material === "metallic"
      ? lighten(card.color, 0.13 * swingScale(card.color))
      : card.material === "soft"
        ? lighten(card.color, 0.05)
        : card.color;
  return compositeOver(surface, alphaOf(card), background);
}

/**
 * 按基色亮度自适应地提亮/压暗，并带上部件透明度。
 * amount 是「满幅时的比例」，实际幅度再乘 swingScale。
 */
function shade(hex: string, amount: number, a: number): string {
  const scaled = amount * swingScale(hex);
  const moved = scaled >= 0 ? lighten(hex, scaled) : darken(hex, -scaled);
  return withAlpha(moved, a);
}

function gradient(angle: string, stops: Array<[string, number]>): string {
  return `linear-gradient(${angle}, ${stops.map(([c, p]) => `${c} ${p}%`).join(", ")})`;
}

/* ---------------------------------- 背景 --------------------------------- */

function backgroundVars(bg: BackgroundSpec, url?: string | null): Vars {
  const a = alphaOf(bg);
  const dim = Math.max(0, Math.min(70, bg.dim)) / 100;
  const { r, g, b } = hexToRgb(bg.color);
  // 「背景不透明度」在纯色上没有意义（时钟铺满整屏，底下没有东西可透）。
  // 它实际控制的是图片的浓淡：100 图片清晰，30 只剩浅浅一层。
  // 做法是用底色盖一层 veils，而不是给图片设 opacity —— 后者会让整块背景一起变淡。
  const veil = Number((1 - a).toFixed(3));
  return {
    "--clock-bg": bg.color,
    "--clock-glow": shade(bg.color, 0.22, 0.34),
    "--background-image": url ? `url("${url}")` : "none",
    "--background-size": bg.fit === "tile" ? "auto" : "cover",
    "--background-repeat": bg.fit === "tile" ? "repeat" : "no-repeat",
    "--background-veil": `linear-gradient(rgba(${r}, ${g}, ${b}, ${veil}), rgba(${r}, ${g}, ${b}, ${veil}))`,
    "--background-dim": `linear-gradient(rgba(0, 0, 0, ${dim}), rgba(0, 0, 0, ${dim}))`,
  };
}

/* ---------------------------------- 卡片 --------------------------------- */

function cardVars(card: MaterialPart): Vars {
  const c = card.color;
  const a = alphaOf(card);
  // 边线色默认从主色提亮；近黑卡（黑标）必须显式给一个金边，
  // 否则提亮近黑只会得到一圈灰线。
  const line = card.lineColor ?? lighten(c, 0.5 * swingScale(c));
  const lineScale = card.lineColor ? swingScale(card.lineColor) : swingScale(c);

  if (card.material === "metallic") {
    return {
      "--card-top-fill": gradient("135deg", [
        [shade(c, 0.66, a), 0],
        [shade(c, 0.02, a), 18],
        [shade(c, 0.42, a), 37],
        [shade(c, -0.24, a), 60],
        [shade(c, 0.38, a), 82],
        [shade(c, -0.44, a), 100],
      ]),
      "--card-bottom-fill": gradient("135deg", [
        [shade(c, -0.05, a), 0],
        [shade(c, 0.45, a), 23],
        [shade(c, -0.2, a), 48],
        [shade(c, 0.3, a), 70],
        [shade(c, -0.4, a), 100],
      ]),
      "--card-top": withAlpha(lighten(c, 0.42 * swingScale(c)), a),
      "--card-top-end": withAlpha(lighten(c, 0.05 * swingScale(c)), a),
      "--card-bottom-start": withAlpha(c, a),
      "--card-bottom": withAlpha(darken(c, 0.25 * swingScale(c)), a),
      "--card-edge": withAlpha(line, 0.75 * a),
      "--card-inset-display": "block",
      "--card-inset-ring": withAlpha(lighten(line, 0.15 * lineScale), 0.12 * a),
      "--card-bevel": `rgba(255, 255, 255, ${(0.7 * a).toFixed(2)})`,
      "--card-shadow": [
        // 全部长度用 em，em 解析的是 .flip-card 的字号 `--card-font`（定义在 FlipClock，
        // 不是这里）。全屏/屏保把它钉在 13px——改造前从页面继承的就是 13px，所以真机的
        // 阴影仍是恒定像素；预览把它按卡宽换算，否则 18px 的模糊落在 33px 的卡上会糊成
        // 一团光。以前这里是 1px / -7px / 13px 的定值，缩略卡上等于把阴影放大七倍。
        `0 0.6em 1.4em rgba(0, 0, 0, ${(0.42 * a).toFixed(2)})`,
        `inset 0 0.077em 0.154em rgba(255, 255, 255, ${(0.7 * a).toFixed(2)})`,
        `inset 0 -0.538em 1em rgba(0, 0, 0, ${(0.32 * a).toFixed(2)})`,
      ].join(", "),
    };
  }

  if (card.material === "soft") {
    // 柔光同样用固定幅度：它的卖点是「大跨度低对比」，被 swingScale 压缩后
    // 就退化成一块纯色，用户会以为材质切换没生效。
    return {
      "--card-top-fill": gradient("180deg", [
        [withAlpha(lighten(c, 0.16), a), 0],
        [withAlpha(darken(c, 0.06), a), 100],
      ]),
      "--card-bottom-fill": gradient("180deg", [
        [withAlpha(darken(c, 0.04), a), 0],
        [withAlpha(darken(c, 0.14), a), 100],
      ]),
      "--card-top": withAlpha(lighten(c, 0.16), a),
      "--card-top-end": withAlpha(darken(c, 0.06), a),
      "--card-bottom-start": withAlpha(darken(c, 0.04), a),
      "--card-bottom": withAlpha(darken(c, 0.14), a),
      "--card-edge": withAlpha(c, 0),
      "--card-inset-display": "none",
      "--card-inset-ring": withAlpha(c, 0),
      "--card-bevel": `rgba(255, 255, 255, ${(0.1 * a).toFixed(2)})`,
      "--card-shadow": `0 0.5em 1.6em rgba(0, 0, 0, ${(0.2 * a).toFixed(2)})`,
    };
  }

  // matte：两色微渐变，没有金属反光也没有描边。
  // 这里用固定的绝对幅度，不乘 swingScale：哑光是往暗处走，不存在「暗色被洗白」
  // 的问题，而 swingScale 会把近黑主题的渐变压得几乎看不出来（实测卡片下半
  // 比原版亮 7–10 个色阶，OLED 纯黑会显出发灰）。
  return {
    // 起点就是基色本身：内置主题里卡片顶部原本就是那个颜色，
    // 再提亮会让深灰主题变成中灰。
    "--card-top-fill": gradient("180deg", [
      [withAlpha(c, a), 0],
      [withAlpha(darken(c, 0.06), a), 100],
    ]),
    "--card-bottom-fill": gradient("180deg", [
      [withAlpha(darken(c, 0.12), a), 0],
      [withAlpha(darken(c, 0.22), a), 100],
    ]),
    "--card-top": withAlpha(c, a),
    "--card-top-end": withAlpha(darken(c, 0.06), a),
    "--card-bottom-start": withAlpha(darken(c, 0.12), a),
    "--card-bottom": withAlpha(darken(c, 0.22), a),
    "--card-edge": withAlpha(c, 0),
    "--card-inset-display": "none",
    "--card-inset-ring": withAlpha(c, 0),
    "--card-bevel": `rgba(255, 255, 255, ${(0.07 * a).toFixed(2)})`,
    "--card-shadow": `0 0.6em 1.4em rgba(0, 0, 0, ${(0.32 * a).toFixed(2)})`,
  };
}

/* ---------------------------------- 数字 --------------------------------- */

function digitVars(
  digit: DigitSpec,
  card: MaterialPart,
  background: string,
  url?: string | null,
): Vars {
  const c = digit.color;
  const a = alphaOf(digit);
  // 纯色与渐变靠颜色自带的 alpha 表达半透明；只有纹理图才需要元素级 opacity。
  const vars: Vars = { "--digit": withAlpha(c, a), "--digit-opacity": "1" };

  if (url) {
    // 纹理图铺满每个数字。图片没法「带 alpha 的颜色」，所以透明度走元素 opacity：
    // 半透明的纹理字本来就该连投影一起变淡，这与纯色/渐变走颜色 alpha 不同。
    vars["--digit-fill"] = `url("${url}")`;
    vars["--digit-clip"] = "text";
    vars["--digit-ink"] = "transparent";
    vars["--digit-opacity"] = a.toFixed(2);
    vars["--digit-shadow-layer"] = "none";
    vars["--digit-filter"] = "drop-shadow(0 1px 2px rgba(0, 0, 0, 0.35))";
    vars["--digit-stroke-width"] = "0";
    vars["--digit-stroke-color"] = "transparent";
    vars["--digit-paint-order"] = "normal";
    return vars;
  }

  if (digit.material === "metallic") {
    // 数字比卡片亮 → 凸起（顶光打在倒角上）；比卡片暗 → 压印（凹槽投影）。
    // 比较对象是卡面实际渲染出来的颜色，含材质提亮与卡片透明度。
    const surface = cardSurfaceColor(card, background);
    const raised = luminance(c) > luminance(surface);
    vars["--digit-fill"] = raised
      ? gradient("155deg", [
          [shade(c, 0.5, a), 6],
          [shade(c, 0.12, a), 24],
          [shade(c, -0.12, a), 44],
          [shade(c, 0.28, a), 50],
          [shade(c, 0.45, a), 58],
          [shade(c, -0.18, a), 78],
          [shade(c, 0.12, a), 88],
          [shade(c, -0.32, a), 97],
        ])
      : gradient("180deg", [
          [shade(c, 0.15, a), 6],
          [shade(c, -0.12, a), 22],
          [shade(c, -0.32, a), 40],
          [shade(c, 0.12, a), 54],
          [shade(c, 0.2, a), 58],
          [shade(c, -0.35, a), 76],
          [shade(c, -0.2, a), 90],
        ]);
    vars["--digit-clip"] = "text";
    vars["--digit-ink"] = "transparent";
    vars["--digit-shadow-layer"] = "none";
    vars["--digit-filter"] = raised
      ? `drop-shadow(0 -1px 0 ${withAlpha(lighten(c, 0.6 * swingScale(c)), 0.55 * a)}) drop-shadow(0 2px 3px rgba(0, 0, 0, ${(0.68 * a).toFixed(2)}))`
      : `drop-shadow(0 -1px 0 rgba(0, 0, 0, ${(0.5 * a).toFixed(2)})) drop-shadow(0 1px 0 ${withAlpha(lighten(c, 0.55 * swingScale(c)), 0.42 * a)})`;
    vars["--digit-stroke-width"] = "0.5px";
    vars["--digit-stroke-color"] = withAlpha(
      lighten(c, 0.55 * swingScale(c)),
      (raised ? 0.38 : 0.22) * a,
    );
    vars["--digit-paint-order"] = "stroke fill";
    return vars;
  }

  if (digit.material === "soft") {
    vars["--digit-fill"] = gradient("180deg", [
      [shade(c, 0.2, a), 0],
      [withAlpha(c, a), 50],
      [shade(c, -0.18, a), 100],
    ]);
    vars["--digit-clip"] = "text";
    vars["--digit-ink"] = "transparent";
    vars["--digit-shadow-layer"] = "none";
    vars["--digit-filter"] = `drop-shadow(0 1px 2px rgba(0, 0, 0, ${(0.25 * a).toFixed(2)}))`;
    vars["--digit-stroke-width"] = "0";
    vars["--digit-stroke-color"] = "transparent";
    vars["--digit-paint-order"] = "normal";
    return vars;
  }

  // matte：实色，只留一道极细的投影保证在卡片上立得住。
  vars["--digit-fill"] = "none";
  vars["--digit-clip"] = "border-box";
  vars["--digit-ink"] = withAlpha(c, a);
  vars["--digit-shadow-layer"] = `0 1px ${withAlpha(darken(card.color, 0.4), 0.25 * a)}`;
  vars["--digit-filter"] = "none";
  vars["--digit-stroke-width"] = "0";
  vars["--digit-stroke-color"] = "transparent";
  vars["--digit-paint-order"] = "normal";
  return vars;
}

/* ------------------------------- 日期与冒号 ------------------------------ */

function dateVars(date: FlatPart, material: DigitSpec["material"]): Vars {
  const c = date.color;
  const a = alphaOf(date);

  if (material === "metallic") {
    // 日期继承数字的材质，但字号小：色标少、描边细，避免糊成一团。
    return {
      "--date-color": withAlpha(c, a),
      "--date-fill": gradient("155deg", [
        [shade(c, 0.3, a), 0],
        [shade(c, -0.1, a), 42],
        [shade(c, 0.2, a), 70],
        [shade(c, -0.25, a), 100],
      ]),
      "--date-clip": "text",
      "--date-ink": "transparent",
      "--date-stroke-width": "0.25px",
      "--date-stroke-color": withAlpha(lighten(c, 0.55 * swingScale(c)), 0.25 * a),
      "--date-paint-order": "stroke fill",
    };
  }

  if (material === "soft") {
    return {
      "--date-color": withAlpha(c, a),
      "--date-fill": gradient("180deg", [[shade(c, 0.18, a), 0], [shade(c, -0.12, a), 100]]),
      "--date-clip": "text",
      "--date-ink": "transparent",
      "--date-stroke-width": "0",
      "--date-stroke-color": "transparent",
      "--date-paint-order": "normal",
    };
  }

  return {
    "--date-color": withAlpha(c, a),
    "--date-fill": "none",
    "--date-clip": "border-box",
    "--date-ink": withAlpha(c, a),
    "--date-stroke-width": "0",
    "--date-stroke-color": "transparent",
    "--date-paint-order": "normal",
  };
}

/**
 * 翻页卡中间那条横向的缝。
 *
 * 原本它由卡片色推导（更暗一档），但那样卡片调到全透明时缝还留着一条，
 * 数字中间会浮出一条线。改成独立部件后，「无缝」和「要缝但换颜色」都能表达。
 * 缝下方那道极细的高光仍从缝的色提亮，保持两者同族。
 */
function seamVars(seam: FlatPart): Vars {
  const a = alphaOf(seam);
  return {
    "--seam": withAlpha(seam.color, a),
    "--seam-light": withAlpha(lighten(seam.color, 0.82), 0.08 * a),
  };
}

function separatorVars(sep: FlatPart): Vars {
  return {
    "--separator-color": withAlpha(sep.color, alphaOf(sep)),
    // 原本固定 0.62 的透明度并入部件透明度，避免两处相乘。
    "--separator-opacity": "1",
  };
}

/* ---------------------------------- 入口 --------------------------------- */

export function buildThemeVars(spec: ThemeSpec, assets: ThemeAssetUrls = {}): Vars {
  return {
    ...backgroundVars(spec.background, assets.background),
    ...cardVars(spec.card),
    ...digitVars(spec.digit, spec.card, spec.background.color, assets.digit),
    ...dateVars(spec.date, spec.digit.material),
    ...seamVars(spec.seam),
    ...separatorVars(spec.separator),
    // 圆角：金属卡偏锐，其余保持原标定。
    "--card-radius":
      spec.card.material === "metallic" ? "calc(var(--radius) * 0.78)" : "var(--radius)",
  };
}
