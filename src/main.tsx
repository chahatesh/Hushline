// SPDX-License-Identifier: MIT
import { render } from 'solid-js/web';
import { App, Overlay } from './ui';
import './styles.css';
render(() => new URLSearchParams(location.search).has('overlay') ? <Overlay /> : <App />, document.getElementById('root')!);
