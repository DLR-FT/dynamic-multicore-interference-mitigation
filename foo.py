import marimo

__generated_with = "0.23.9"
app = marimo.App()


@app.cell
def _():
    import json
    import pandas as pd
    import plotly.express as px

    def read_trace32_printftrace(f):
        text = "".join([l[13:].rstrip() for l in f.readlines()[2:]])

        decoder = json.JSONDecoder()
        data = []
        while text:
            obj, idx = decoder.raw_decode(text)
            data.append(obj)
            text = text[idx:].lstrip()
    
        return data

    return pd, px, read_trace32_printftrace


@app.cell
def _(pd, read_trace32_printftrace):
    with open("foo.txt") as f:
        data = read_trace32_printftrace(f)

    data = pd.json_normalize(data)

    data

    return (data,)


@app.cell
def _(data, px):
    px.box(data, x="intruder_set_mask", y="perf_info.l2d_refill")
    return


@app.cell
def _(data, px):
    px.box(data, x="intruder_set_mask", y="dt")
    return


@app.cell
def _():
    return


if __name__ == "__main__":
    app.run()
