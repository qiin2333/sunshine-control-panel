import { createApp } from 'vue'
import '../style.css'
import App from './index.vue'
import {
  ElButton,
  ElForm,
  ElFormItem,
  ElInput,
  ElInputNumber,
  ElOption,
  ElSelect,
  ElSwitch,
  ElTag,
} from 'element-plus'
import 'element-plus/theme-chalk/base.css'
import 'element-plus/theme-chalk/el-button.css'
import 'element-plus/theme-chalk/el-form.css'
import 'element-plus/theme-chalk/el-form-item.css'
import 'element-plus/theme-chalk/el-input.css'
import 'element-plus/theme-chalk/el-input-number.css'
import 'element-plus/theme-chalk/el-message.css'
import 'element-plus/theme-chalk/el-option.css'
import 'element-plus/theme-chalk/el-popper.css'
import 'element-plus/theme-chalk/el-scrollbar.css'
import 'element-plus/theme-chalk/el-select.css'
import 'element-plus/theme-chalk/el-select-dropdown.css'
import 'element-plus/theme-chalk/el-switch.css'
import 'element-plus/theme-chalk/el-tag.css'
// 导入 Tauri polyfill
import '../tauri-polyfill.js'

const app = createApp(App)
app.component(ElButton.name, ElButton)
app.component(ElForm.name, ElForm)
app.component(ElFormItem.name, ElFormItem)
app.component(ElInput.name, ElInput)
app.component(ElInputNumber.name, ElInputNumber)
app.component(ElOption.name, ElOption)
app.component(ElSelect.name, ElSelect)
app.component(ElSwitch.name, ElSwitch)
app.component(ElTag.name, ElTag)
app.mount('#app')
