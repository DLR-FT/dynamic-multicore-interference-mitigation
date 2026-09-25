import marimo

__generated_with = "0.23.16"
app = marimo.App()


@app.cell
def _():
    import json
    import pandas as pd
    import plotly.express as px
    import plotly.graph_objects as go

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
    with open("foo-3.txt") as f:
        data = read_trace32_printftrace(f)

    data = pd.json_normalize(data)


    data["intruder_set_mask"] = data["intruder_set_mask"].astype(str)

    data["ipf"] = data["perf_info.instr"] / data["df"]

    data["cpi"] = data["perf_info.cycles"] / data["perf_info.instr"]
    data["api"] = data["perf_info.l1d_access"] / data["perf_info.instr"]

    data["l1_miss_ratio"] = data["perf_info.l2d_access"] / data["perf_info.l1d_access"]
    data["l2_miss_ratio"] = data["perf_info.l2d_refill"] / data["perf_info.l2d_access"]

    dt_baseline = data[data["intruder_set_mask"] == "0"]["dt"].median()
    data["rel_impact"] = data["dt"] / dt_baseline

    data
    return (data,)


@app.cell
def _(data, px):
    px.scatter(data, x="l2_miss_ratio", y="rel_impact", color="intruder_set_mask")
    return


@app.cell
def _(data, px):
    x = data.groupby("intruder_set_mask").agg({"l2_miss_ratio": "median", "rel_impact": "median"}).reset_index()
    px.scatter(x, x="l2_miss_ratio", y="rel_impact", color="intruder_set_mask")
    return


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
 
    """)
    return


@app.cell
def _(data, px):

    px.scatter(data, x="intruder_set_mask", y="rel_impact", color="intruder_set_mask")
    return


@app.cell
def _(data, px):
    px.scatter(data, x="l2_miss_ratio", y="l1_miss_ratio", color="intruder_set_mask")
    return


@app.cell
def _(data, px):
    px.box(data, x="api", color="intruder_set_mask")
    return


@app.cell
def _():
    return


if __name__ == "__main__":
    app.run()
