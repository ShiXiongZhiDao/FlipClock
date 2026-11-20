import { createApp } from "vue";
import App from "./App.vue";

import "./styles/tokens.css";
import "./styles/themes.css";
import "./styles/fonts.css";
import "./styles/global.css";
import "./styles/ui.css";

/**
 * 全软件禁用原生右键菜单。
 *
 * 放在入口而不是某个视图里：所有窗口（设置、全屏配置、屏保、时钟预览）都走这份
 * 入口，一处覆盖全部形态。用捕获阶段确保先于组件内的监听处理。
 *
 * 屏保视图原先自己抑制了一次右键，这里已经统一接管，它只保留 dragstart 抑制。
 * 取舍见 docs/adr/0024：输入框里的右键复制/粘贴一并没了，Ctrl+C/V 不受影响。
 */
document.addEventListener("contextmenu", (event) => event.preventDefault(), true);

createApp(App).mount("#app");
