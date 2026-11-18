export type AsyncTask = () => Promise<unknown>;

export declare function allSettledConcurrent<T extends readonly AsyncTask[]>(
    ...tasks: T
): Promise<{
    [K in keyof T]: T[K] extends AsyncTask
        ? PromiseSettledResult<Awaited<ReturnType<T[K]>>>
        : never;
}>;
