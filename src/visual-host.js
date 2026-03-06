import './app.css'
import VisualHost from './visual-host/VisualHost.svelte'
import { mount } from 'svelte'

const app = mount(VisualHost, { target: document.getElementById('app') })

export default app
