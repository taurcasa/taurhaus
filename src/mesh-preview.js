import './app.css'
import MeshPreview from './MeshPreview.svelte'
import { mount } from 'svelte'

mount(MeshPreview, { target: document.getElementById('app') })
