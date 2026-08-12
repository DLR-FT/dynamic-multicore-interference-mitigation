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
    with open("foo4.txt") as f:
        data = read_trace32_printftrace(f)

    data = pd.json_normalize(data)

    data["cpi"] = data["perf_info.cycles"] / data["perf_info.instr"]
    data["l1_miss_ratio"] = data["perf_info.l1d_refill"] / data["perf_info.l1d_access"]
    data["l2_miss_ratio"] = data["perf_info.l2d_refill"] / data["perf_info.l1d_refill"]

    dt_baseline = data[data["intruder_set_mask"] == 0]["dt"].median()
    data["rel_impact"] = data["dt"] / dt_baseline

    data
    return (data,)


@app.cell
def _(data, px):
    px.scatter(data, x="l1_miss_ratio", y="rel_impact")
    return


@app.cell
def _(data, px):
    px.scatter(data, x="l2_miss_ratio", y="rel_impact")
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
 
    """)
    return


@app.cell
def _(data, px):

    px.scatter(data, x="l2_miss_ratio", y="cpi")
    return


@app.cell
def _(data):
    b = data[data["intruder_set_mask"] == 0x3FF]
    b
    return (b,)


@app.cell
def _(b):
    (b["perf_info.l1d_refill"] / b["perf_info.l1d_access"]).median()
    return


@app.cell
def _():
    return


if __name__ == "__main__":
    app.run()
