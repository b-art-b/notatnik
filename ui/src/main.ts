import './assets/fonts/inter.css'
import { mount } from 'svelte'
import App from './App.svelte'

mount(App, { target: document.getElementById('app')! })

// A file dropped anywhere but on an editor would make the webview open it and leave the app (ST-8).
for (const type of ['dragover', 'drop']) {
  window.addEventListener(type, (e) => {
    if ((e as DragEvent).dataTransfer?.types.includes('Files')) e.preventDefault()
  })
}
