/**
 * Starts independent requests together and returns every result, including failures.
 * @param {...(() => Promise<unknown>)} tasks
 * @returns {Promise<PromiseSettledResult<unknown>[]>}
 */
export function allSettledConcurrent(...tasks) {
    return Promise.allSettled(tasks.map((task) => task()));
}
