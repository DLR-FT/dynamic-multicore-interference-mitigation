import marimo

__generated_with = "0.23.9"
app = marimo.App()


@app.cell
def _():
    import pandas as pd
    import json


    def read_trace32_printftrace(f):
        text = "".join([l[13:].rstrip() for l in f.readlines()[2:]])

        decoder = json.JSONDecoder()
        data = []
        while text:
            obj, idx = decoder.raw_decode(text)
            data.append(obj)
            text = text[idx:].lstrip()
    
        return data

    return (read_trace32_printftrace,)


@app.cell
def _(read_trace32_printftrace):
    with open("foo.txt") as f:
        data = read_trace32_printftrace(f)

    data
    
    return


@app.cell
def _():
    return


if __name__ == "__main__":
    app.run()
