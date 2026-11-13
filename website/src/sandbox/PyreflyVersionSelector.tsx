/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

import React, { useEffect, useState } from 'react';
import * as stylex from '@stylexjs/stylex';
import {
    fetchPyreflyVersions,
    type PyreflyVersionInfo,
} from './pyreflyVersions';

export interface PyreflyVersionSelectorProps {
    /** The selected version, or `null` for the bundled latest build. */
    selected: string | null;
    onSelect: (version: string | null) => void;
    disabled: boolean;
}

export default function PyreflyVersionSelector({
    selected,
    onSelect,
    disabled,
}: PyreflyVersionSelectorProps): React.ReactElement {
    // The version list loads when the sandbox mounts. A failure leaves only
    // the bundled build, which still works.
    const [versions, setVersions] = useState<PyreflyVersionInfo[] | null>(null);

    useEffect(() => {
        let cancelled = false;
        fetchPyreflyVersions()
            .then((loaded) => {
                if (!cancelled) {
                    setVersions(loaded);
                }
            })
            .catch(() => {
                if (!cancelled) {
                    setVersions([]);
                }
            });
        return () => {
            cancelled = true;
        };
    }, []);

    const releases = versions?.filter((info) => !info.prerelease) ?? [];
    const prereleases = versions?.filter((info) => info.prerelease) ?? [];
    // A selected version that the list does not know, from a shared URL or
    // while the list loads, stays selectable.
    const extraSelected =
        selected !== null &&
        !versions?.some((info) => info.version === selected)
            ? selected
            : null;

    return (
        <select
            id="pyrefly-version-selector"
            aria-label="Select Pyrefly version"
            {...stylex.props(styles.versionSelector)}
            value={selected ?? ''}
            onChange={(event) =>
                onSelect(event.target.value === '' ? null : event.target.value)
            }
            disabled={disabled}
            title={
                versions === null
                    ? 'Loading the Pyrefly version list'
                    : versions.length === 0
                      ? 'The Pyrefly version list is unavailable'
                      : undefined
            }
        >
            <option value="">Latest (bundled build)</option>
            {extraSelected !== null && (
                <option value={extraSelected}>Pyrefly {extraSelected}</option>
            )}
            {releases.length > 0 && (
                <optgroup label="Releases">
                    {releases.map((info) => (
                        <option key={info.version} value={info.version}>
                            Pyrefly {info.version}
                        </option>
                    ))}
                </optgroup>
            )}
            {prereleases.length > 0 && (
                <optgroup label="Dev builds">
                    {prereleases.map((info) => (
                        <option key={info.version} value={info.version}>
                            Pyrefly {info.version}
                        </option>
                    ))}
                </optgroup>
            )}
        </select>
    );
}

const styles = stylex.create({
    versionSelector: {
        border: 'none',
        background: 'transparent',
        color: 'var(--color-text)',
        fontFamily: 'inherit',
        fontSize: '12px',
        padding: '7px 10px',
        cursor: 'pointer',
        outline: 'none',
        maxWidth: '220px',
        ':disabled': {
            opacity: 0.4,
            cursor: 'not-allowed',
        },
    },
});
