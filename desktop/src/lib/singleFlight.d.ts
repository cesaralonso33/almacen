export declare class SingleFlight {
    get running(): boolean;
    run<T>(task: () => Promise<T>): Promise<T>;
}
