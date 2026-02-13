import { createApp } from 'vue';
import './index.css';
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css';
import VueVirtualScroller from 'vue-virtual-scroller';
import App from './App.vue';

const app = createApp(App);
app.use(VueVirtualScroller);
app.mount('#app');
