// SPDX-License-Identifier: MIT
import { createSignal, Show } from 'solid-js';
import Download from 'lucide-solid/icons/download';
import { version } from '../package.json';
import { call, desktop, errorText } from './bridge';

type UpdateStatus = {
  availableVersion: string | null;
  publishedRelease: boolean;
  canInstall: boolean;
};

export function UpdatePanel() {
  const [status, setStatus] = createSignal<UpdateStatus>();
  const [operation, setOperation] = createSignal<'check' | 'install'>();
  const [error, setError] = createSignal('');

  async function check() {
    setOperation('check');
    setStatus(undefined);
    setError('');
    try { setStatus(await call<UpdateStatus>('check_for_update')); }
    catch (cause) { setError(errorText(cause)); }
    finally { setOperation(undefined); }
  }

  async function install(availableVersion: string) {
    setOperation('install');
    setError('');
    try { await call('install_update', { version: availableVersion }); }
    catch (cause) { setError(errorText(cause)); setOperation(undefined); }
  }

  return <section class="panel settings-panel" aria-labelledby="updates-heading">
    <h2 id="updates-heading">Software updates</h2>
    <p>Hush {version} · Stable releases from GitHub</p>
    <p>Updates restart Hush and keep your settings, history, and saved recordings.</p>
    <div class="button-row">
      <button class="secondary" disabled={!desktop || !!operation()} onClick={check}>
        {operation() === 'check' ? 'Checking…' : 'Check for updates'}
      </button>
      <Show when={status()?.canInstall && status()?.availableVersion} keyed>{availableVersion =>
        <button class="primary" disabled={!!operation()} onClick={() => install(availableVersion)}>
          <Download size={16}/>{operation() === 'install' ? 'Verifying update…' : `Install Hush ${availableVersion}`}
        </button>
      }</Show>
    </div>
    <Show when={!desktop}><p class="field-note">Open Hush on Windows to check for updates.</p></Show>
    <Show when={status()} keyed>{result => <p class="field-note" role="status">
      {result.availableVersion
        ? result.canInstall
          ? `Hush ${result.availableVersion} is available. Finish any active dictation before installing.`
          : <>Hush {result.availableVersion} is available. <a href="https://github.com/chahatesh/Hushline/releases" target="_blank" rel="noreferrer">Get the update on GitHub.</a></>
        : result.publishedRelease ? 'You’re on the latest version.' : 'There are no published Hush releases yet.'}
    </p>}</Show>
    <Show when={error()}><p class="field-note" role="alert">{error()}</p></Show>
  </section>;
}
