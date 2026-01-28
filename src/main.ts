// 應用入口點，啟動 Vue 應用，掛載核心元件
import { createApp } from 'vue';
import './index.css';
import App from './App.vue';

// 掛載主組件至 app root
document.addEventListener('DOMContentLoaded', () => {
  createApp(App).mount('#app');
});
