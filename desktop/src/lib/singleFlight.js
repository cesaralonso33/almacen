/** Shares one in-flight asynchronous operation among concurrent callers. */
export class SingleFlight {
    #inFlight = null;

    get running() {
        return this.#inFlight !== null;
    }

    run(task) {
        if (this.#inFlight) return this.#inFlight;

        const work = Promise.resolve().then(task);
        let tracked;
        tracked = work.finally(() => {
            if (this.#inFlight === tracked) this.#inFlight = null;
        });
        this.#inFlight = tracked;
        return tracked;
    }
}
