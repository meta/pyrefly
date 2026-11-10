/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 * @format
 */

// Jest runs modules as CommonJS, where `import.meta` is a syntax error. This
// plugin replaces `import.meta` with an object whose `url` is the module file.
module.exports = ({ template }) => ({
    visitor: {
        MetaProperty(path) {
            if (path.node.meta.name !== 'import') {
                return;
            }
            path.replaceWith(
                template.expression.ast`({ url: require('url').pathToFileURL(__filename).href })`
            );
        },
    },
});
