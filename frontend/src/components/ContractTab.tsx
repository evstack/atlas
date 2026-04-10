import { useEffect, useMemo, useRef, useState } from 'react';
import type { ContractDetail, AbiItem, VerifyContractRequest } from '../types';
import { verifyContract } from '../api/contracts';
import {
  CUSTOM_OPTIMIZER_RUN_PRESET,
  EVM_VERSION_OPTIONS,
  LICENSE_OPTIONS,
  OPTIMIZER_RUN_PRESETS,
  SOLC_VERSION_OPTIONS,
} from '../constants/contractVerification';

const themedInputClassName =
  'w-full bg-dark-700/80 backdrop-blur border border-dark-500 px-3 py-2 text-sm text-fg placeholder-gray-500 rounded-xl shadow-md shadow-black/20 focus:outline-none focus:border-accent-primary focus:ring-2 focus:ring-accent-primary/40 transition';

const themedMonoInputClassName = `${themedInputClassName} font-mono`;
const themedTextareaClassName = `${themedMonoInputClassName} resize-y min-h-[22rem]`;

interface Props {
  address: string;
  contract: ContractDetail | null;
  loading: boolean;
  onVerified: () => void;
}

export default function ContractTab({ address, contract, loading, onVerified }: Props) {
  if (loading) {
    return <div className="text-gray-400 text-sm py-8 text-center">Loading…</div>;
  }

  if (contract?.verified) {
    return <VerifiedView contract={contract} />;
  }

  return <VerifyForm address={address} onVerified={onVerified} />;
}

// ── Verification form ─────────────────────────────────────────────────────────

interface VerifyFormProps {
  address: string;
  onVerified: () => void;
}

function VerifyForm({ address, onVerified }: VerifyFormProps) {
  const [compilerVersion, setCompilerVersion] = useState('');
  const [contractName, setContractName] = useState('');
  const [mode, setMode] = useState<'single' | 'multi'>('single');
  const [sourceCode, setSourceCode] = useState('');
  const [sourceFiles, setSourceFiles] = useState<{ name: string; content: string }[]>([]);
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [optimizationEnabled, setOptimizationEnabled] = useState(false);
  const [optimizationRunsPreset, setOptimizationRunsPreset] = useState<string>('200');
  const [customOptimizationRuns, setCustomOptimizationRuns] = useState('');
  const [constructorArgs, setConstructorArgs] = useState('');
  const [evmVersion, setEvmVersion] = useState('');
  const [licenseType, setLicenseType] = useState('');

  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function switchMode(next: 'single' | 'multi') {
    setMode(next);
    setError(null);
    if (next === 'multi') {
      setSourceCode('');
      setSourceFiles([]);
    } else {
      setSourceFiles([]);
    }
  }

  function removeFile(index: number) {
    setSourceFiles(prev => prev.filter((_, i) => i !== index));
  }

  async function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const picked = Array.from(e.target.files ?? []);
    if (picked.length === 0) return;
    const loaded = await Promise.all(
      picked.map(file =>
        new Promise<{ name: string; content: string }>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve({ name: file.name, content: reader.result as string });
          reader.onerror = () => reject(new Error(`Failed to read ${file.name}`));
          reader.readAsText(file);
        }),
      ),
    );
    setSourceFiles(prev => {
      const existing = new Map(prev.map(f => [f.name, f]));
      for (const f of loaded) existing.set(f.name, f);
      return Array.from(existing.values());
    });
    // Reset input so the same files can be re-added after removal
    e.target.value = '';
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!compilerVersion.trim()) {
      setError('Select a compiler version.');
      return;
    }

    if (mode === 'multi') {
      if (sourceFiles.length === 0) {
        setError('Add at least one .sol file.');
        return;
      }
    }

    setSubmitting(true);
    setError(null);

    const optimizationRunsValue =
      optimizationRunsPreset === CUSTOM_OPTIMIZER_RUN_PRESET
        ? customOptimizationRuns
        : optimizationRunsPreset;

    const req: VerifyContractRequest = mode === 'single'
      ? {
          source_code: sourceCode,
          compiler_version: compilerVersion.trim(),
          optimization_enabled: optimizationEnabled,
          optimization_runs: optimizationEnabled ? parseInt(optimizationRunsValue, 10) || 200 : undefined,
          contract_name: contractName.trim(),
          constructor_args: constructorArgs.trim() || undefined,
          evm_version: evmVersion.trim() || undefined,
          license_type: licenseType.trim() || undefined,
        }
      : {
          source_files: Object.fromEntries(sourceFiles.map(f => [f.name.trim(), f.content])),
          compiler_version: compilerVersion.trim(),
          optimization_enabled: optimizationEnabled,
          optimization_runs: optimizationEnabled ? parseInt(optimizationRunsValue, 10) || 200 : undefined,
          contract_name: contractName.trim(),
          constructor_args: constructorArgs.trim() || undefined,
          evm_version: evmVersion.trim() || undefined,
          license_type: licenseType.trim() || undefined,
        };

    try {
      await verifyContract(address, req);
      onVerified();
    } catch (err: unknown) {
      const e = err as { error?: string };
      setError(e?.error ?? 'Verification failed');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="py-4">
      <p className="text-gray-400 text-sm mb-6">
        Submit the Solidity source code and compiler settings to verify this contract. The backend
        will compile and compare the bytecode against what is deployed on-chain.
      </p>

      {error && (
        <div className="mb-4 px-4 py-3 bg-red-900/30 border border-red-700 text-red-300 text-sm rounded">
          {error}
        </div>
      )}

      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <label className="flex flex-col gap-1">
            <span className="text-sm text-gray-400">Contract Name <span className="text-red-400">*</span></span>
            <input
              className={themedInputClassName}
              placeholder="e.g. MyToken"
              value={contractName}
              onChange={e => setContractName(e.target.value)}
              required
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-sm text-gray-400">Compiler Version <span className="text-red-400">*</span></span>
            <SearchableOptionSelect
              options={SOLC_VERSION_OPTIONS.map(version => ({ value: version, label: version }))}
              value={compilerVersion}
              onChange={setCompilerVersion}
              placeholder="Search compiler version"
              emptyMessage="No compiler versions found"
              monospace
            />
            <span className="text-xs text-gray-500">Official Solidity compiler releases, newest first.</span>
          </label>
        </div>

        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-1">
            <button
              type="button"
              onClick={() => switchMode('single')}
              className={`px-3 py-1 text-sm border rounded-l-lg ${
                mode === 'single'
                  ? 'border-accent-primary text-accent-primary bg-accent-primary/10'
                  : 'border-dark-500 text-gray-400 hover:border-gray-400'
              }`}
            >
              Single file
            </button>
            <button
              type="button"
              onClick={() => switchMode('multi')}
              className={`px-3 py-1 text-sm border rounded-r-lg ${
                mode === 'multi'
                  ? 'border-accent-primary text-accent-primary bg-accent-primary/10'
                  : 'border-dark-500 text-gray-400 hover:border-gray-400'
              }`}
            >
              Multi-file
            </button>
          </div>

          {mode === 'single' ? (
            <label className="flex flex-col gap-1">
              <span className="text-sm text-gray-400">Solidity Source Code <span className="text-red-400">*</span></span>
              <textarea
                className={themedTextareaClassName}
                rows={14}
                placeholder="// SPDX-License-Identifier: MIT&#10;pragma solidity ^0.8.0;&#10;&#10;contract MyToken { ... }"
                value={sourceCode}
                onChange={e => setSourceCode(e.target.value)}
                required
              />
              <span className="text-xs text-gray-500">Paste a flattened Solidity file (all imports merged).</span>
            </label>
          ) : (
            <div className="flex flex-col gap-3">
              <span className="text-sm text-gray-400">Source Files <span className="text-red-400">*</span></span>
              <input
                ref={fileInputRef}
                type="file"
                accept=".sol"
                multiple
                className="hidden"
                onChange={handleFileChange}
              />
              <button
                type="button"
                onClick={() => fileInputRef.current?.click()}
                className="flex items-center gap-2 self-start px-4 py-2 text-sm border border-dark-500 rounded-xl text-gray-300 hover:border-gray-400 hover:text-fg bg-dark-700/80 backdrop-blur shadow-md shadow-black/20"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                </svg>
                {sourceFiles.length === 0 ? 'Select .sol files' : 'Add more files'}
              </button>
              {sourceFiles.length > 0 && (
                <ul className="flex flex-col gap-1">
                  {sourceFiles.map((file, index) => (
                    <li key={file.name} className="flex items-center justify-between px-3 py-2 bg-dark-700/60 border border-dark-500 rounded-lg text-sm font-mono text-gray-300">
                      <span>{file.name}</span>
                      <button
                        type="button"
                        onClick={() => removeFile(index)}
                        className="text-gray-500 hover:text-red-400 ml-4 shrink-0"
                      >
                        Remove
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              <span className="text-xs text-gray-500">Select all .sol files that make up the contract (imports included).</span>
            </div>
          )}
        </div>

        <div className="flex items-center gap-4">
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={optimizationEnabled}
              onChange={e => setOptimizationEnabled(e.target.checked)}
              className="accent-accent"
            />
            <span className="text-sm text-gray-300">Optimization enabled</span>
          </label>
          {optimizationEnabled && (
            <div className="flex items-center gap-3 flex-wrap">
              <label className="flex items-center gap-2">
                <span className="text-sm text-gray-400">Runs</span>
                <select
                  className="bg-dark-700 text-fg border border-dark-500 px-2 py-1 text-sm min-w-32 focus:outline-none focus:border-accent"
                  value={optimizationRunsPreset}
                  onChange={e => setOptimizationRunsPreset(e.target.value)}
                >
                  {OPTIMIZER_RUN_PRESETS.map(runs => (
                    <option key={runs} value={runs}>
                      {runs}
                    </option>
                  ))}
                  <option value={CUSTOM_OPTIMIZER_RUN_PRESET}>Custom</option>
                </select>
              </label>
              {optimizationRunsPreset === CUSTOM_OPTIMIZER_RUN_PRESET && (
                <label className="flex items-center gap-2">
                  <span className="text-sm text-gray-400">Custom</span>
                  <input
                    className={`${themedInputClassName} w-24 px-2 py-1`}
                    type="number"
                    min={1}
                    value={customOptimizationRuns}
                    onChange={e => setCustomOptimizationRuns(e.target.value)}
                    placeholder="200"
                  />
                </label>
              )}
            </div>
          )}
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <label className="flex flex-col gap-1">
            <span className="text-sm text-gray-400">Constructor Arguments <span className="text-gray-500">(hex, optional)</span></span>
            <input
              className={themedMonoInputClassName}
              placeholder="0x..."
              value={constructorArgs}
              onChange={e => setConstructorArgs(e.target.value)}
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-sm text-gray-400">EVM Version <span className="text-gray-500">(optional)</span></span>
            <SearchableOptionSelect
              options={EVM_VERSION_OPTIONS.map(version => ({ value: version, label: version }))}
              value={evmVersion}
              onChange={setEvmVersion}
              placeholder="Compiler default or search EVM version"
              emptyMessage="No EVM versions found"
              emptyLabel="Compiler default"
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-sm text-gray-400">License <span className="text-gray-500">(optional)</span></span>
            <SearchableOptionSelect
              options={LICENSE_OPTIONS.map(license => ({ value: license, label: license }))}
              value={licenseType}
              onChange={setLicenseType}
              placeholder="Search license"
              emptyMessage="No licenses found"
            />
          </label>
        </div>

        <div className="pt-2">
          <button
            type="submit"
            disabled={submitting}
            className="btn btn-primary disabled:opacity-60 disabled:cursor-not-allowed"
          >
            {submitting ? 'Verifying… (this may take a minute)' : 'Verify Contract'}
          </button>
        </div>
      </form>
    </div>
  );
}

interface SearchableOptionSelectProps {
  options: ReadonlyArray<{ value: string; label: string }>;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  emptyMessage: string;
  monospace?: boolean;
  emptyLabel?: string;
}

function SearchableOptionSelect({
  options,
  value,
  onChange,
  placeholder,
  emptyMessage,
  monospace = false,
  emptyLabel,
}: SearchableOptionSelectProps) {
  const [draftQuery, setDraftQuery] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(-1);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const selectedOption = useMemo(
    () => options.find(option => option.value === value),
    [options, value],
  );
  const query = draftQuery ?? selectedOption?.label ?? '';

  useEffect(() => {
    function handlePointerDown(event: MouseEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
        setHighlight(-1);
        setDraftQuery(null);
      }
    }

    document.addEventListener('mousedown', handlePointerDown);
    return () => document.removeEventListener('mousedown', handlePointerDown);
  }, []);

  const filteredOptions = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return options;
    return options.filter(option =>
      option.label.toLowerCase().includes(normalized) || option.value.toLowerCase().includes(normalized),
    );
  }, [options, query]);

  const displayOptions = useMemo(() => {
    if (emptyLabel && query.trim() === '') {
      return [{ value: '', label: emptyLabel }, ...filteredOptions];
    }
    return filteredOptions;
  }, [emptyLabel, filteredOptions, query]);

  function selectOption(option: { value: string; label: string }) {
    onChange(option.value);
    setDraftQuery(null);
    setOpen(false);
    setHighlight(-1);
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (displayOptions.length === 0) {
      if (e.key === 'Escape') {
        setOpen(false);
        setHighlight(-1);
        setDraftQuery(null);
      }
      return;
    }

    if (!open && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault();
      setOpen(true);
      setHighlight(0);
      return;
    }

    if (!open) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlight(index => (index + 1) % displayOptions.length);
      return;
    }

    if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlight(index => (index - 1 + displayOptions.length) % displayOptions.length);
      return;
    }

    if (e.key === 'Enter' && highlight >= 0 && displayOptions[highlight]) {
      e.preventDefault();
      selectOption(displayOptions[highlight]);
      return;
    }

    if (e.key === 'Escape') {
      setOpen(false);
      setHighlight(-1);
      setDraftQuery(null);
    }
  }

  return (
    <div ref={rootRef} className="relative">
      <input
        type="text"
        value={query}
        onChange={(e) => {
          setDraftQuery(e.target.value);
          setOpen(true);
          setHighlight(0);
        }}
        onFocus={() => {
          setOpen(true);
          setHighlight(0);
        }}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        className={`${monospace ? themedMonoInputClassName : themedInputClassName} pl-10`}
      />
      <svg
        className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500 pointer-events-none"
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
        />
      </svg>

      {open && (
        <div className="absolute left-0 right-0 mt-2 bg-dark-800/95 border border-dark-600 rounded-xl shadow-xl overflow-hidden z-40">
          <ul role="listbox" className="max-h-72 overflow-y-auto">
            {displayOptions.length === 0 && (
              <li className="px-3 py-2 text-sm text-gray-400 select-none cursor-default">{emptyMessage}</li>
            )}
            {displayOptions.map((option, idx) => (
              <li
                key={`${option.value || '__empty__'}-${option.label}`}
                role="option"
                aria-selected={highlight === idx}
                className={`px-3 py-2 cursor-pointer text-sm ${monospace ? 'font-mono' : ''} ${
                  highlight === idx ? 'bg-dark-700/70 text-fg' : 'text-gray-300 hover:bg-dark-700/40'
                }`}
                onMouseEnter={() => setHighlight(idx)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => selectOption(option)}
              >
                {option.label}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

// ── Verified view ─────────────────────────────────────────────────────────────

interface VerifiedViewProps {
  contract: ContractDetail;
}

function VerifiedView({ contract }: VerifiedViewProps) {
  const [abiExpanded, setAbiExpanded] = useState(false);
  const [activeFile, setActiveFile] = useState<string | null>(null);

  const files = contract.is_multi_file && contract.source_files
    ? contract.source_files
    : null;
  const firstFile = files ? Object.keys(files)[0] : null;
  const displayFile = activeFile ?? firstFile;
  const displaySource = files && displayFile ? files[displayFile] : contract.source_code;

  return (
    <div className="py-4 space-y-6">
      {/* Compiler info */}
      <div className="flex flex-wrap gap-3 text-sm">
        {contract.contract_name && (
          <span className="badge-chip">{contract.contract_name}</span>
        )}
        {contract.compiler_version && (
          <span className="badge-chip font-mono">{contract.compiler_version}</span>
        )}
        {contract.optimization_used !== undefined && (
          <span className="badge-chip">
            Optimization: {contract.optimization_used ? `on (${contract.runs ?? 200} runs)` : 'off'}
          </span>
        )}
        {contract.evm_version && (
          <span className="badge-chip">EVM: {contract.evm_version}</span>
        )}
        {contract.license_type && (
          <span className="badge-chip">{contract.license_type}</span>
        )}
        {contract.verified_at && (
          <span className="text-gray-500 text-xs self-center">
            Verified {new Date(contract.verified_at).toLocaleDateString()}
          </span>
        )}
      </div>

      {/* Source code */}
      {(displaySource || files) && (
        <div>
          <div className="flex items-center justify-between mb-2">
            <h3 className="text-sm font-semibold text-gray-300 uppercase tracking-wide">Source Code</h3>
          </div>

          {files && (
            <div className="flex flex-wrap gap-1 mb-2">
              {Object.keys(files).map(filename => (
                <button
                  key={filename}
                  onClick={() => setActiveFile(filename)}
                  className={`px-2 py-1 text-xs border ${
                    displayFile === filename
                      ? 'border-accent text-accent'
                      : 'border-dark-500 text-gray-400 hover:border-gray-400'
                  }`}
                >
                  {filename}
                </button>
              ))}
            </div>
          )}

          <pre className="bg-dark-700 border border-dark-500 p-4 text-xs font-mono text-gray-200 overflow-x-auto max-h-96 overflow-y-auto whitespace-pre">
            {displaySource}
          </pre>
        </div>
      )}

      {/* ABI viewer */}
      {contract.abi && contract.abi.length > 0 && (
        <div>
          <button
            className="flex items-center gap-2 text-sm font-semibold text-gray-300 uppercase tracking-wide mb-2 hover:text-fg"
            onClick={() => setAbiExpanded(v => !v)}
          >
            <span>ABI</span>
            <span className="text-gray-500 font-normal">({contract.abi.length} items)</span>
            <span className="text-gray-500">{abiExpanded ? '▲' : '▼'}</span>
          </button>

          {abiExpanded && <AbiViewer abi={contract.abi} />}
        </div>
      )}
    </div>
  );
}

// ── ABI viewer ────────────────────────────────────────────────────────────────

function AbiViewer({ abi }: { abi: AbiItem[] }) {
  const functions = abi.filter(item => item.type === 'function');
  const events = abi.filter(item => item.type === 'event');
  const constructors = abi.filter(item => item.type === 'constructor');
  const other = abi.filter(item => !['function', 'event', 'constructor'].includes(item.type));

  return (
    <div className="border border-dark-500 divide-y divide-dark-500 text-sm">
      {constructors.map((item, i) => (
        <AbiRow key={`constructor-${i}`} item={item} />
      ))}
      {functions.map((item, i) => (
        <AbiRow key={`fn-${i}`} item={item} />
      ))}
      {events.map((item, i) => (
        <AbiRow key={`ev-${i}`} item={item} />
      ))}
      {other.map((item, i) => (
        <AbiRow key={`other-${i}`} item={item} />
      ))}
    </div>
  );
}

function AbiRow({ item }: { item: AbiItem }) {
  const inputs = item.inputs ?? [];
  const outputs = item.outputs ?? [];
  const inputSig = inputs.map(i => `${i.type}${i.name ? ' ' + i.name : ''}`).join(', ');
  const outputSig = outputs.map(o => o.type).join(', ');

  const typeColor =
    item.type === 'function' ? 'text-blue-400'
    : item.type === 'event' ? 'text-yellow-400'
    : item.type === 'constructor' ? 'text-green-400'
    : 'text-gray-400';

  return (
    <div className="px-4 py-2 font-mono text-xs text-gray-200 flex flex-wrap gap-x-2 items-baseline">
      <span className={`${typeColor} shrink-0`}>{item.type}</span>
      {item.name && <span className="text-fg font-medium">{item.name}</span>}
      <span className="text-gray-500">({inputSig})</span>
      {item.stateMutability && (
        <span className="text-gray-500 text-[10px]">{item.stateMutability}</span>
      )}
      {outputSig && <span className="text-gray-500">→ {outputSig}</span>}
    </div>
  );
}
