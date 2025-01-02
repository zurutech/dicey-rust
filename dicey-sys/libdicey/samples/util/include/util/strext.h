/*
 * Copyright (c) 2014-2024 Zuru Tech HK Limited, All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#if !defined(XVAUPCELRC_STREXT_H)
#define XVAUPCELRC_STREXT_H

#include <string.h>

#if defined(_MSC_VER) && __STDC_VERSION__ < 202311L

static inline char *strndup(const char *const str, const size_t maxlen) {
    const size_t len = strnlen(str, maxlen);

    char *const ret = malloc(len + 1);
    if (ret) {
        memcpy(ret, str, len);
        ret[len] = '\0';
    }

    return ret;
}

#endif // _MSC_VER && __STDC_VERSION__ < 202311L

#endif // XVAUPCELRC_STREXT_H
